# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.6.4](https://github.com/Xevion/Banner/compare/v0.6.3...v0.6.4) (2026-02-06)


### Features

* **log:** Enhance tracing formatter with syntax highlighting and smart field grouping ([8d58f8a](https://github.com/Xevion/Banner/commit/8d58f8a6c78cfeb8ae5eedcbd7a7614bcc9fd8c2))
* **stream:** Add ScraperStats, ScraperTimeseries, and ScraperSubjects stream kinds ([f32b727](https://github.com/Xevion/Banner/commit/f32b72754c0ab36161b97df56e65b33d8a78e693))
* **terms:** Add multi-term scraping support with database table and admin API ([f5d13b0](https://github.com/Xevion/Banner/commit/f5d13b072a45ba01e540e330b0555dfc5e66f5b8))


### Bug Fixes

* **ci:** Update bytes dependency for security audit fix, add security audits to check script ([049938a](https://github.com/Xevion/Banner/commit/049938a0a670414ac3e30ac178473ea05777118a))
* **deps:** Update time to 0.3.47 for RUSTSEC-2026-0009 ([1321d1e](https://github.com/Xevion/Banner/commit/1321d1e2d9f6de8d8646b41dde45441785a33610))


### Code Refactoring

* **db:** Restructure database layer with context and event emission ([5551dc3](https://github.com/Xevion/Banner/commit/5551dc381f07aa7371726c9fbd3d9b012226d13b))
* **middleware:** Consolidate rate limiting into unified module ([a69ba89](https://github.com/Xevion/Banner/commit/a69ba8964c0653fa82acd9eba6db03a7caec59aa))
* **query:** Replace Duration with NaiveTime for course search time parameters ([6252777](https://github.com/Xevion/Banner/commit/6252777b2946e6a8a987e880b00e75932c8a7879))
* **tracing:** Improve log messages across codebase, standardize duration formatting ([0f230e6](https://github.com/Xevion/Banner/commit/0f230e677b0922791952a795935bf33d56e03a90))
* **web:** Consolidate admin pages into tabbed scraper interface ([6fb7d79](https://github.com/Xevion/Banner/commit/6fb7d7913ac108e59ee5f937608844b8e2ab1ac5))

## [0.6.3](https://github.com/Xevion/Banner/compare/v0.6.2...v0.6.3) (2026-02-03)


### Features

* **web:** Add related sections API and refactor course view ([bec81f7](https://github.com/Xevion/Banner/commit/bec81f74ba3e8ab80a1a00675cc10a916c02f4aa))
* **web:** Add storybook with component stories and vitest integration ([5cb3155](https://github.com/Xevion/Banner/commit/5cb3155c8847f29fba346f25692039c19a271645))
* **web:** Enable ESLint with TypeScript and Svelte support ([87f0808](https://github.com/Xevion/Banner/commit/87f0808eadb518ccd068132e2565c999b1a2ff7a))


### Bug Fixes

* Outdated course data queries ([bd9a8d3](https://github.com/Xevion/Banner/commit/bd9a8d32aa1d4b4f9ad08159b9cfd708a4d5fe2f))
* Switch weekdays jsonb query to modern array format ([348492f](https://github.com/Xevion/Banner/commit/348492f1911d85ad15b9b958970b05cffb8c18f9))
* **web:** Resolve animation rendering issues with fill modes ([a71e648](https://github.com/Xevion/Banner/commit/a71e64821d1117a6b8b228904afce87af3daf798))


### Code Refactoring

* **api:** Migrate to typed reference data enums ([56fabc2](https://github.com/Xevion/Banner/commit/56fabc22214d62e77d423cbdcdd3083be834afd2))
* Consolidate course data models into structured types ([2157035](https://github.com/Xevion/Banner/commit/215703593b6e6696f2dc478bd29644374fa1e787))
* **web:** Centralize filter state with context-based store ([f4a3c55](https://github.com/Xevion/Banner/commit/f4a3c5521a518b4226065652712f71b772e605f8))
* **web:** Fix ESLint violations and enhance types ([89f3a23](https://github.com/Xevion/Banner/commit/89f3a23fa792d0ce5df8439b3efb038cd7c75301))


### Miscellaneous

* **web:** Disable clearScreen for vite dev server ([c5fef57](https://github.com/Xevion/Banner/commit/c5fef573b4b3dacc87514afc4908eee2cd86d4a0))

## [0.6.2](https://github.com/Xevion/Banner/compare/v0.6.1...v0.6.2) (2026-02-01)


### Features

* **web:** Add dynamic range sliders with consolidated search options API ([f5a639e](https://github.com/Xevion/Banner/commit/f5a639e88bfe03dfc635f25e06fc22208ee0c855))
* **web:** Batch rapid search query changes into history entries, allow for query history ([e920968](https://github.com/Xevion/Banner/commit/e9209684eb051f978607a31f237b19e883af5d5a))
* **web:** Build responsive layout with mobile card view ([bd2acee](https://github.com/Xevion/Banner/commit/bd2acee6f40c0768898ab39e0524c0474ec4fd31))
* **web:** Implement aligned course codes with jetbrains mono ([567c4ae](https://github.com/Xevion/Banner/commit/567c4aec3ca7baaeb548fff2005d83f7e6228d79))
* **web:** Implement multi-dimensional course filtering system ([106bf23](https://github.com/Xevion/Banner/commit/106bf232c4b53f4ca8902a582f185e146878c54e))
* **web:** Implement smooth view transitions for search results ([5729a82](https://github.com/Xevion/Banner/commit/5729a821d54d95a00e9f4ba736a2bd884c0c409b))


### Bug Fixes

* **cli:** Add proper flag validation for check script ([2acf52a](https://github.com/Xevion/Banner/commit/2acf52a63b6dcd24ca826b99061bf7a51a9230b1))
* **data:** Handle alphanumeric course numbers in range filtering ([96a8c13](https://github.com/Xevion/Banner/commit/96a8c13125428f1cc14e46d8f580719c17c029ef))
* Re-add overflow hidden for page transitions, but with negative margin padding to avoid clipping ([9e825cd](https://github.com/Xevion/Banner/commit/9e825cd113bbc65c10f0386b5300b6aec50bf936))
* Separate Biome format and lint checks to enable auto-format ([ac8dbb2](https://github.com/Xevion/Banner/commit/ac8dbb2eefe79ec5d898cfa719e270f4713125d5))
* **web:** Ignore .svelte-kit/generated in vite watcher ([b562fe2](https://github.com/Xevion/Banner/commit/b562fe227e89a0826fe4587372e3eeca2ab6eb33))
* **web:** Prevent duplicate searches and background fetching on navigation ([5dd35ed](https://github.com/Xevion/Banner/commit/5dd35ed215d3d1f3603e67a2aa59eaddf619f5c9))
* **web:** Prevent interaction blocking during search transitions ([7f0f087](https://github.com/Xevion/Banner/commit/7f0f08725a668c5ac88c510f43791d90ce2f795e))
* **web:** Skip view transitions for same-page navigations ([b37604f](https://github.com/Xevion/Banner/commit/b37604f8071741017a83f74a67b73cf7975827ae))


### Code Refactoring

* **api:** Extract toURLSearchParams helper for query param handling ([6c15f40](https://github.com/Xevion/Banner/commit/6c15f4082f1a4b6fb6c54c545c6e0ec47e191654))
* **api:** Rename middleware and enable database query logging ([f387401](https://github.com/Xevion/Banner/commit/f387401a4174d4d0bdf74deccdda80b3af543b74))
* Migrate API responses from manual JSON to type-safe bindings ([0ee4e8a](https://github.com/Xevion/Banner/commit/0ee4e8a8bc1fe0b079fea84ac303674083b43a59))
* Standardize error responses with ApiError and ts-rs bindings ([239f7ee](https://github.com/Xevion/Banner/commit/239f7ee38cbc0e49d9041579fc9923fd4a4608bf))
* **web:** Consolidate tooltip implementations with shared components ([d91f7ab](https://github.com/Xevion/Banner/commit/d91f7ab34299b26dc12d629bf99d502ee05e7cfa))
* **web:** Extract FilterPopover component and upgrade range sliders ([4e01406](https://github.com/Xevion/Banner/commit/4e0140693b00686e8a57561b0811fdf25a614e65))
* **web:** Replace component tooltips with delegated singleton ([d278498](https://github.com/Xevion/Banner/commit/d278498daa4afc82c877b536ecd1264970dc92a7))
* **web:** Split CourseTable into modular component structure ([bbff2b7](https://github.com/Xevion/Banner/commit/bbff2b7f36744808b62ec130be2cfbdc96f87b69))
* **web:** Streamline filter ui with simplified removal ([4426042](https://github.com/Xevion/Banner/commit/44260422d68e910ed4ad37e78cd8a1d1f8bb51a3))


### Miscellaneous

* Add aliases to Justfile ([02b18f0](https://github.com/Xevion/Banner/commit/02b18f0c66dc8b876452f35999c027475df52462))
* Add dev-build flag for embedded vite builds ([5134ae9](https://github.com/Xevion/Banner/commit/5134ae93881854ac722dc9e7f3f5040aee3e517a))

## [0.6.1](https://github.com/Xevion/Banner/compare/v0.6.0...v0.6.1) (2026-01-31)


### Features

* **build:** Auto-regenerate TypeScript bindings on source changes ([e203e8e](https://github.com/Xevion/Banner/commit/e203e8e182f7a0b0224a8f9e6bf79d15259215a2))
* **course:** Distinguish async from synchronous online courses ([8bfc14e](https://github.com/Xevion/Banner/commit/8bfc14e55c1bdf5acc2006096476e0b1eb1b7cc6))
* **scraper:** Improve dashboard clarity with stat tooltips ([1ad614d](https://github.com/Xevion/Banner/commit/1ad614dad03d3631a8d119203786718c814e72c7))
* **scraper:** Improve results visibility and loading states ([c533768](https://github.com/Xevion/Banner/commit/c53376836238f3aca92ac82cd5fd59a077bcceff))


### Bug Fixes

* Avoid status flickering on subjects table ([2689587](https://github.com/Xevion/Banner/commit/2689587dd53c572a65eeb91f74c737662e1f148b))
* **ci:** Add postgres container service for rust tests ([ebb7a97](https://github.com/Xevion/Banner/commit/ebb7a97c113fa1d4b61b8637dfe97cae5260075c))
* **ci:** Fix rust/frontend/security job failures and expand local checks ([dd148e0](https://github.com/Xevion/Banner/commit/dd148e08a0b6d5b7afe4ff614d7d6e4e4d0dfce6))
* **data:** Decode HTML entities in course titles and instructor names ([7d2255a](https://github.com/Xevion/Banner/commit/7d2255a988a23f6e1b1c8e7cb5a8ead833ad34da))
* **metrics:** Always emit baseline metrics on initial course insertion ([16039e0](https://github.com/Xevion/Banner/commit/16039e02a999c668d4969a43eb9ed1d4e8d370e1))


### Code Refactoring

* **terms:** Move term formatting from frontend to backend ([cbb0a51](https://github.com/Xevion/Banner/commit/cbb0a51bca9e4e0d6a8fcee90465c93943f2a30e))
* Use friendly term codes in URL query parameters ([550401b](https://github.com/Xevion/Banner/commit/550401b85ceb8a447e316209b479c69062c5b658))


### Continuous Integration

* Add Release Please automation for changelog and version management ([6863ee5](https://github.com/Xevion/Banner/commit/6863ee58d0a5778303af1b7626b2a9eda3043ca0))
* Split quality checks into parallel jobs with security scanning ([3494341](https://github.com/Xevion/Banner/commit/3494341e3fbe9ffd96b6fcd8abbe7f95ecec6f45))


### Miscellaneous

* Add ts-rs generated bindings ([2df0ba0](https://github.com/Xevion/Banner/commit/2df0ba0ec58155d73830a66132cb635dc819e8a9))
* Update frontend packages ([acccaa5](https://github.com/Xevion/Banner/commit/acccaa54d4455500db60d1b6437cad1c592445f1))

## [Unreleased]

## [0.6.0] - 2026-01-30

### Added

- User authentication system with Discord OAuth, sessions, admin roles, and login page with FAQ.
- Interactive timeline visualization with D3 canvas, pan/zoom, touch gestures, and enrollment aggregation API.
- Scraper analytics dashboard with timeseries charts, subject monitoring, and per-subject detail views.
- Adaptive scraper scheduling with admin endpoints for monitoring and configuration.
- Scrape job result persistence for effectiveness tracking.
- WebSocket support for real-time scrape job monitoring with connection status indicators.
- Course change auditing with field-level tracking and time-series metrics endpoint.
- Audit log UI with smart JSON diffing, conditional request caching, and auto-refresh.
- Calendar export web endpoints for ICS download and Google Calendar redirect.
- Confidence-based RMP matching with manual review workflow and admin instructor UI.
- RMP profile links and confidence-aware rating display.
- Name parsing and normalization for improved instructor-RMP matching.
- Mobile touch controls with gesture detection for timeline.
- Worker timeout protection and crash recovery for job queue.
- Build-time asset compression with encoding negotiation (gzip, brotli, zstd).
- Smart page transitions with theme-aware element transitions.
- Search duration and result count feedback.
- Root error page handling.
- Login page with FAQ section and improved styling.

### Changed

- Consolidated navigation with top nav bar and route groups.
- Centralized number formatting with locale-aware utility.
- Modernized Justfile commands and simplified service management.
- Persisted audit log state in module scope for cross-navigation caching.
- Relative time feedback and improved tooltip customization.

### Fixed

- Instructor/course mismatching via build-order-independent map for association.
- Page content clipping.
- Backend startup delays with retry logic in auth.
- Banner API timeouts increased to handle slow responses.
- i64 serialization for JavaScript compatibility, fixing avatar URL display.
- Frontend build ordering with `-e` embed flag in Justfile.
- Login page centering and unnecessary scrollbar.
- ts-rs serde warnings.

## [0.5.0] - 2026-01-29

### Added

- Multi-select subject filtering with searchable comboboxes.
- Smart instructor name abbreviation for compact table display.
- Delivery mode indicators and tooltips in location column.
- Page selector dropdown with animated pagination controls.
- FLIP animations for smooth table row transitions during pagination.
- Time tooltip with detailed meeting schedule and day abbreviations.
- Reusable SimpleTooltip component for consistent UI hints.

### Changed

- Consolidated query logic and eliminated N+1 instructor loads via batch fetching.
- Consolidated menu snippets and strengthened component type safety.
- Enhanced table scrolling with OverlayScrollbars and theme-aware styling.
- Eliminated initial theme flash on page load.

## [0.4.0] - 2026-01-28

### Added

- Web-based course search UI with interactive data table, multi-column sorting, and column visibility controls.
- TypeScript type bindings generated from Rust types via ts-rs.
- RateMyProfessors integration: bulk professor sync via GraphQL and inline rating display in search results.
- Course detail expansion panel with enrollment, meeting times, and instructor info.
- OverlayScrollbars integration for styled, theme-aware scrollable areas.
- Pagination component for navigating large search result sets.
- Footer component with version display.
- API endpoints: `/api/courses/search`, `/api/courses/:term/:crn`, `/api/terms`, `/api/subjects`, `/api/reference/:category`.
- Frontend API client with typed request/response handling and test coverage.
- Course formatting utilities with comprehensive unit tests.

## [0.3.4] - 2026-01

### Added

- Live service status tracking on web dashboard with auto-refresh and health indicators.
- DB operation extraction for improved testability.
- Unit test suite foundation covering core functionality.
- Docker support for PostgreSQL development environment.
- ICS calendar export with comprehensive holiday exclusion coverage.
- Google Calendar link generation with recurrence rules and meeting details.
- Job queue with priority-based scheduling for background scraping.
- Rate limiting with burst allowance for Banner API requests.
- Session management and caching for Banner API interactions.
- Discord bot commands: search, terms, ics, gcal.
- Intelligent scraping system with priority queues and retry tracking.

### Changed

- Type consolidation and dead code removal across the codebase.
