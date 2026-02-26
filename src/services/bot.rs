use super::Service;
use crate::bot::{Data, get_commands};
use crate::state::AppState;
use crate::state::{ServiceStatus, ServiceStatusRegistry};
use num_format::{Locale, ToFormattedString};
use serenity::Client;
use serenity::all::{ActivityData, ClientBuilder, GatewayIntents};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

/// Discord bot service implementation
pub struct BotService {
    bot_token: String,
    bot_target_guild: u64,
    app_state: AppState,
    status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Consumed once in `run()` and passed into `create_client`.
    status_shutdown_rx: Option<broadcast::Receiver<()>>,
    status_shutdown_tx: Option<broadcast::Sender<()>>,
    service_statuses: ServiceStatusRegistry,
    /// Populated inside `run()` after the Discord client is built.
    shard_manager: Arc<Mutex<Option<Arc<serenity::gateway::ShardManager>>>>,
}

impl BotService {
    pub fn new(
        bot_token: String,
        bot_target_guild: u64,
        app_state: AppState,
        status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
        status_shutdown_tx: broadcast::Sender<()>,
        status_shutdown_rx: broadcast::Receiver<()>,
        service_statuses: ServiceStatusRegistry,
    ) -> Self {
        Self {
            bot_token,
            bot_target_guild,
            app_state,
            status_task_handle,
            status_shutdown_rx: Some(status_shutdown_rx),
            status_shutdown_tx: Some(status_shutdown_tx),
            service_statuses,
            shard_manager: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new Discord bot client with full configuration
    async fn create_client(
        bot_token: &str,
        bot_target_guild: u64,
        app_state: AppState,
        status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
        status_shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Client, anyhow::Error> {
        let intents = GatewayIntents::non_privileged();

        let framework = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: get_commands(),
                pre_command: |ctx| {
                    Box::pin(async move {
                        let content = match ctx {
                            poise::Context::Application(_) => ctx.invocation_string(),
                            poise::Context::Prefix(prefix) => prefix.msg.content.to_string(),
                        };
                        let channel_name = ctx
                            .channel_id()
                            .name(ctx.http())
                            .await
                            .unwrap_or("unknown".to_string());

                        let span = tracing::Span::current();
                        span.record("command_name", ctx.command().qualified_name.as_str());
                        span.record("invocation", ctx.invocation_string());
                        span.record("msg.content", content.as_str());
                        span.record("msg.author", ctx.author().tag().as_str());
                        span.record("msg.id", ctx.id());
                        span.record("msg.channel_id", ctx.channel_id().get());
                        span.record("msg.channel", channel_name.as_str());

                        tracing::info!(
                            command_name = ctx.command().qualified_name.as_str(),
                            invocation = ctx.invocation_string(),
                            msg.content = %content,
                            msg.author = %ctx.author().tag(),
                            msg.author_id = %ctx.author().id,
                            msg.id = %ctx.id(),
                            msg.channel = %channel_name.as_str(),
                            msg.channel_id = %ctx.channel_id(),
                            "{} invoked by {}",
                            ctx.command().name,
                            ctx.author().tag()
                        );
                    })
                },
                on_error: |error| {
                    Box::pin(async move {
                        if let Err(e) = poise::builtins::on_error(error).await {
                            tracing::error!(error = %e, "Fatal error while sending error message");
                        }
                    })
                },
                ..Default::default()
            })
            .setup(move |ctx, ready, framework| {
                let app_state = app_state.clone();
                let status_task_handle = status_task_handle.clone();
                Box::pin(async move {
                    let command_count = framework.options().commands.len();
                    info!(
                        username = %ready.user.name,
                        user_id = %ready.user.id,
                        guilds = ready.guilds.len(),
                        shard_count = ready.shard.map(|s| s.total).unwrap_or(1),
                        commands = command_count,
                        "Discord bot connected and ready"
                    );

                    // Skip command registration if definitions haven't changed since last startup.
                    let fingerprint =
                        crate::bot::commands_fingerprint(&framework.options().commands);
                    let stored =
                        crate::data::kv::get(&app_state.db_pool, "bot.commands_fingerprint")
                            .await
                            .unwrap_or(None);

                    if stored.as_deref() == Some(fingerprint.as_str()) {
                        info!(
                            commands = command_count,
                            "Discord commands unchanged, skipping registration"
                        );
                    } else {
                        poise::builtins::register_in_guild(
                            ctx,
                            &framework.options().commands,
                            bot_target_guild.into(),
                        )
                        .await?;
                        poise::builtins::register_globally(ctx, &framework.options().commands)
                            .await?;

                        if let Err(e) = crate::data::kv::set(
                            &app_state.db_pool,
                            "bot.commands_fingerprint",
                            &fingerprint,
                        )
                        .await
                        {
                            warn!(error = ?e, "Failed to persist command fingerprint");
                        }

                        info!(
                            guild_commands = command_count,
                            global_commands = command_count,
                            target_guild = %bot_target_guild,
                            "Discord commands registered"
                        );
                    }

                    // Start status update task with shutdown support
                    let handle = Self::start_status_update_task(
                        ctx.clone(),
                        app_state.clone(),
                        status_shutdown_rx,
                    );
                    *status_task_handle.lock().await = Some(handle);

                    app_state.service_statuses.set("bot", ServiceStatus::Active);

                    Ok(Data { app_state })
                })
            })
            .build();

        Ok(ClientBuilder::new(bot_token, intents)
            .framework(framework)
            .await?)
    }

    /// Start the status update task for the Discord bot with graceful shutdown support
    fn start_status_update_task(
        ctx: serenity::client::Context,
        app_state: AppState,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let max_interval = Duration::from_secs(300); // 5 minutes
            let base_interval = Duration::from_secs(30);
            let mut interval = tokio::time::interval(base_interval);
            let mut previous_course_count: Option<i64> = None;

            // This runs once immediately on startup, then with adaptive intervals
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Get the course count, update the activity if it has changed/hasn't been set this session
                        let course_count = match app_state.get_course_count().await {
                            Ok(count) => count,
                            Err(e) => {
                                warn!(error = %e, "Failed to fetch course count for status update");
                                continue;
                            }
                        };
                        if previous_course_count.is_none() || previous_course_count != Some(course_count) {
                            ctx.set_activity(Some(ActivityData::playing(format!(
                                "Querying {:} classes",
                                course_count.to_formatted_string(&Locale::en)
                            ))));
                        }

                        // Increase or reset the interval
                        interval = tokio::time::interval(
                            // Avoid logging the first 'change'
                            if course_count != previous_course_count.unwrap_or(0) {
                                if previous_course_count.is_some() {
                                    debug!(
                                        new_course_count = course_count,
                                        last_interval = interval.period().as_secs(),
                                        "Course count changed, resetting interval"
                                    );
                                }

                                // Record the new course count
                                previous_course_count = Some(course_count);

                                // Reset to base interval
                                base_interval
                            } else {
                                // Increase interval by 10% (up to maximum)
                                let new_interval = interval.period().mul_f32(1.1).min(max_interval);
                                trace!(
                                    current_course_count = course_count,
                                    last_interval = interval.period().as_secs(),
                                    new_interval = new_interval.as_secs(),
                                    "Course count unchanged, increasing interval"
                                );

                                new_interval
                            },
                        );

                        // Reset the interval, otherwise it will tick again immediately
                        interval.reset();
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Status update task received shutdown signal");
                        break;
                    }
                }
            }
        })
    }

}

#[async_trait::async_trait]
impl Service for BotService {
    fn name(&self) -> &'static str {
        "bot"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        let status_shutdown_rx = self
            .status_shutdown_rx
            .take()
            .ok_or_else(|| anyhow::anyhow!("BotService::run called more than once"))?;

        let mut client = Self::create_client(
            &self.bot_token,
            self.bot_target_guild,
            self.app_state.clone(),
            self.status_task_handle.clone(),
            status_shutdown_rx,
        )
        .await?;

        *self.shard_manager.lock().await = Some(client.shard_manager.clone());

        match client.start().await {
            Ok(()) => {
                warn!(service = "bot", "stopped early");
                Err(anyhow::anyhow!("bot stopped early"))
            }
            Err(e) => {
                error!(service = "bot", "error: {e:?}");
                Err(e.into())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        self.service_statuses.set("bot", ServiceStatus::Disabled);

        if let Some(status_shutdown_tx) = self.status_shutdown_tx.take() {
            let _ = status_shutdown_tx.send(());
        }

        let handle = self.status_task_handle.lock().await.take();
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_secs(2), handle).await {
                Ok(Ok(())) => {
                    debug!("Status update task completed gracefully");
                }
                Ok(Err(e)) => {
                    warn!(error = ?e, "Status update task panicked");
                }
                Err(_) => {
                    warn!("Status update task did not complete within 2s timeout");
                }
            }
        }

        if let Some(shard_manager) = self.shard_manager.lock().await.take() {
            shard_manager.shutdown_all().await;
        }

        Ok(())
    }
}
