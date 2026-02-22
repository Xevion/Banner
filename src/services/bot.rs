use super::Service;
use crate::bot::{Data, get_commands};
use crate::config::Config;
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
    client: Client,
    shard_manager: Arc<serenity::gateway::ShardManager>,
    status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    status_shutdown_tx: Option<broadcast::Sender<()>>,
    service_statuses: ServiceStatusRegistry,
}

impl BotService {
    /// Create a new Discord bot client with full configuration
    pub async fn create_client(
        config: &Config,
        app_state: AppState,
        status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
        status_shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Client, anyhow::Error> {
        let intents = GatewayIntents::non_privileged();
        let bot_target_guild = config.bot_target_guild;

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

                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands,
                        bot_target_guild.into(),
                    )
                    .await?;
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                    info!(
                        guild_commands = command_count,
                        global_commands = command_count,
                        target_guild = %bot_target_guild,
                        "Discord commands registered"
                    );

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

        Ok(ClientBuilder::new(config.bot_token.clone(), intents)
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

    pub fn new(
        client: Client,
        status_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
        status_shutdown_tx: broadcast::Sender<()>,
        service_statuses: ServiceStatusRegistry,
    ) -> Self {
        let shard_manager = client.shard_manager.clone();

        Self {
            client,
            shard_manager,
            status_task_handle,
            status_shutdown_tx: Some(status_shutdown_tx),
            service_statuses,
        }
    }
}

#[async_trait::async_trait]
impl Service for BotService {
    fn name(&self) -> &'static str {
        "bot"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        match self.client.start().await {
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
        // Signal status update task to stop
        if let Some(status_shutdown_tx) = self.status_shutdown_tx.take() {
            let _ = status_shutdown_tx.send(());
        }

        // Wait for status update task to complete (with timeout)
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

        // Shutdown Discord shards
        self.shard_manager.shutdown_all().await;
        Ok(())
    }
}
