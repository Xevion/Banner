<script lang="ts">
import SiDiscord from "@icons-pack/svelte-simple-icons/icons/SiDiscord";
import { ChevronDown } from "@lucide/svelte";
import { Accordion } from "bits-ui";

const faqItems = [
  {
    value: "what",
    question: "What does this do?",
    answer:
      "Banner monitors UTSA course availability in real-time. Get notified on Discord when seats open up in the classes you need, so you never miss a registration window.",
  },
  {
    value: "why-discord",
    question: "Why sign in with Discord?",
    answer:
      "Banner delivers notifications through a Discord bot. Signing in with Discord lets us link your account to send you alerts directly, and lets you manage your watchlist from the web dashboard.",
  },
  {
    value: "data",
    question: "What data do you access?",
    answer:
      "We only request your Discord username and avatar. We don't read your messages, join servers on your behalf, or access any other Discord data.",
  },
  {
    value: "official",
    question: "Is this an official UTSA tool?",
    answer:
      "No. Banner is an independent, community-built project. It is not affiliated with, endorsed by, or maintained by UTSA or Ellucian.",
  },
];
</script>

<svelte:head>
  <title>Login | Banner</title>
</svelte:head>

<div class="flex flex-1 items-center justify-center px-4 pb-14">
    <div class="w-full max-w-md flex flex-col gap-y-8">
        <!-- Sign-in card -->
        <div
            class="rounded-xl border border-border bg-card p-8 text-center shadow-sm"
        >
            <p class="text-sm text-muted-foreground">
                Sign in to manage your watchlist and notifications.
            </p>

            <a
                href="/api/auth/login"
                class="mt-6 inline-flex w-full items-center justify-center gap-2.5 rounded-lg bg-[#5865F2] px-6 py-3 text-base font-semibold text-white shadow-sm transition-colors hover:bg-[#4752C4] active:bg-[#3C45A5]"
            >
                <SiDiscord size={20} color="white" title="" />
                Sign in with Discord
            </a>
        </div>

        <!-- FAQ section -->
        <div class="flex flex-col gap-y-3">
            <h2
                class="text-center text-xs font-medium uppercase tracking-widest text-muted-foreground"
            >
                Frequently Asked Questions
            </h2>

            <Accordion.Root type="single">
                {#each faqItems as item (item.value)}
                    <Accordion.Item
                        value={item.value}
                        class="border-b border-border last:border-b-0"
                    >
                        <Accordion.Header>
                            <Accordion.Trigger
                                class="group flex w-full items-center justify-between py-3 text-left text-sm font-medium text-foreground/80 transition-colors hover:text-foreground"
                            >
                                {item.question}
                                <ChevronDown
                                    size={16}
                                    class="shrink-0 text-muted-foreground/50 transition-transform duration-200 group-data-[state=open]:rotate-180"
                                />
                            </Accordion.Trigger>
                        </Accordion.Header>
                        <Accordion.Content
                            class="overflow-hidden text-sm leading-relaxed text-muted-foreground/70 data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down"
                        >
                            <div class="pb-3">
                                {item.answer}
                            </div>
                        </Accordion.Content>
                    </Accordion.Item>
                {/each}
            </Accordion.Root>
        </div>
    </div>
</div>
