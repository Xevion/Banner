import type { HandleClientError } from "@sveltejs/kit";
import posthog from "posthog-js";

export const handleError: HandleClientError = ({ error, status }) => {
  const errorId = crypto.randomUUID();
  const timestamp = new Date().toISOString();

  const errorMessage = error instanceof Error ? error.message : String(error);
  const stack = error instanceof Error ? error.stack : undefined;

  if (status !== 404) {
    posthog.captureException(error, {
      $set: { errorId },
    });
  }

  return {
    message: status === 404 ? "Not Found" : errorMessage,
    errorId,
    timestamp,
    stack,
  };
};
