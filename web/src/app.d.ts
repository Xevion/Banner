/// <reference types="@sveltejs/kit" />

declare const __APP_VERSION__: string;

declare namespace App {
  interface Error {
    message: string;
    errorId?: string;
    timestamp?: string;
    stack?: string;
  }
  // interface Locals {}
  // interface PageData {}
  // interface PageState {}
  // interface Platform {}
}
