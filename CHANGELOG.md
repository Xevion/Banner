# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.7.1](https://github.com/Xevion/Banner/compare/v0.7.0...v0.7.1) (2026-02-26)


### Features

* Add Cache-Control headers, ETags, and dynamic robots.txt ([3a39e70](https://github.com/Xevion/Banner/commit/3a39e7062a5cdc05c895a7f19c8068dccb4aff91))
* Add course/subject detail pages, breadcrumb nav, and cross-linking ([6b20697](https://github.com/Xevion/Banner/commit/6b20697abc30618bef4fc466a5f772d292c245d4))
* Add PostHog telemetry with type-safe event tracking ([a46ade5](https://github.com/Xevion/Banner/commit/a46ade510e762786bf7256e11e7be19cd2747ee5))
* Add security headers, CSP reporting, and client IP extraction ([229e0b2](https://github.com/Xevion/Banner/commit/229e0b2d939e9bf15c4c163fb6e74f076401ac6c))
* Add Server header with package version ([bbed357](https://github.com/Xevion/Banner/commit/bbed357249bebf6fb0d1dbfe71c327069175a356))
* Add unsigned newtypes (Count, DurationMs) for non-negative DB fields ([fa5bfcf](https://github.com/Xevion/Banner/commit/fa5bfcfc7d1a50e8d95f88db3bab0d8a709101e9))
* Add XML sitemap endpoints with in-memory caching ([dde5f8b](https://github.com/Xevion/Banner/commit/dde5f8b1c7c7f6e85a5e1f94d7d1cfd344953a15))


### Bug Fixes

* **calendar:** Resolve term slug to code before DB lookup ([91f701e](https://github.com/Xevion/Banner/commit/91f701e72d6822df82da894d133e6799e5026350))
* **csp:** Allow data: URIs in font-src for inline woff2 fonts ([136075c](https://github.com/Xevion/Banner/commit/136075ca98007fb6e9364fd6456a0761c2a4b684))
* **env:** Propagate proper posthog host keys through Dockerfile for CSP ([db7b481](https://github.com/Xevion/Banner/commit/db7b481214a8ba2abd515933e4c00c47f6b376c6))
* Inject x-request-id into request for SSR propagation ([2839b75](https://github.com/Xevion/Banner/commit/2839b7563c93d312027daadc3f7463b4698f528f))
* Propagate x-forwarded-for through SSR proxy, fix PostHog distinctId misuse ([d901a2d](https://github.com/Xevion/Banner/commit/d901a2d59ff54d00c0fe51e9003d7b64b576413e))
* **search:** Disable part-of-term filter for summer terms, prefer section for course breadcrumb ([2537a9c](https://github.com/Xevion/Banner/commit/2537a9ca292d1dafdc49630790584ec7742732ad))

## [0.7.0](https://github.com/Xevion/Banner/compare/v0.6.3...v0.7.0) (2026-02-25)


### Features

* Accent-insensitive search via PostgreSQL unaccent extension ([e6fb3e1](https://github.com/Xevion/Banner/commit/e6fb3e1c18c54b742301eb0580afa9cafcb58612))
* Add BlueBook course evaluation scraper and database schema ([bd2c3c3](https://github.com/Xevion/Banner/commit/bd2c3c34516f1d7b7016cef20a2efd465db41984))
* Add incremental BlueBook scraping with per-subject intervals ([17d205c](https://github.com/Xevion/Banner/commit/17d205cae20773403ac3d682855d54f8e790ec4b))
* Add pending status, nickname matching, and review-course scoring to RMP matching ([7ccfb06](https://github.com/Xevion/Banner/commit/7ccfb063a85ce5b1116abd23f3b2608ea2ae9076))
* Add public instructor directory and profile pages ([eb8c6cb](https://github.com/Xevion/Banner/commit/eb8c6cbbd0335b22dabf24dd0e9038aa50209e7d))
* Add RMP review scraping with per-professor scheduling ([5e18ecf](https://github.com/Xevion/Banner/commit/5e18ecfc8e428dc7e669fc555af28b63ce277c9e))
* Add ScoreBar component with layerchart visualization and Storybook decorators ([c32970b](https://github.com/Xevion/Banner/commit/c32970b904cffd502cdafdf233543376c85a913d))
* Add search autocomplete with server-side trigram suggestions ([616974f](https://github.com/Xevion/Banner/commit/616974f3050da416cf3f5b9d4ec158f0b3e2b34c))
* Add section count to instructor suggestions and fix audit sort order ([2edfe45](https://github.com/Xevion/Banner/commit/2edfe45b4adbe9baa69076fb1d13f21d4e1dcd75))
* **admin:** Add BlueBook instructor linking admin UI and API ([4e1af55](https://github.com/Xevion/Banner/commit/4e1af55f887d57d1f5d895ef34d4ab0cb6da3006))
* **admin:** Add term column to scraper jobs and audit tables ([9964218](https://github.com/Xevion/Banner/commit/9964218e11db1e7fa12c329ee94b5f3e7874f7df))
* **backend:** Add app_kv store to persist scheduler and bot state across restarts ([69090d0](https://github.com/Xevion/Banner/commit/69090d0105d6dd7b1aa139c70c7ced8af289b657))
* **backend:** Add Cache-Control headers to reference and search-options routes ([80f5856](https://github.com/Xevion/Banner/commit/80f58563eee48cfc58ccd04bf7664fedb26f7fdd))
* **backend:** Spawn background reference cache refresh every 30 minutes ([a183776](https://github.com/Xevion/Banner/commit/a18377608dd106629981806c799767edb332afc4))
* Bayesian instructor scoring combining RMP and BlueBook data ([c47bb8c](https://github.com/Xevion/Banner/commit/c47bb8c7851084659573f34d33ae0922acfce598))
* **bot:** Add subject and term autocomplete to search command ([feb0142](https://github.com/Xevion/Banner/commit/feb0142b375201477e9c89990c1e83ccb24be73f))
* **debug:** Add dev auth bypass for local testing ([fdea62c](https://github.com/Xevion/Banner/commit/fdea62cb2e99899fb3ee1c6ca7551076f994b6e0))
* Enrich RMP matching with review metadata and unified subject scoring ([ddcfcd6](https://github.com/Xevion/Banner/commit/ddcfcd67c781f25a404db92baf29d06f5b2ee6e8))
* Extract SortSelect component with direction toggle ([d85206c](https://github.com/Xevion/Banner/commit/d85206cc518bb27a424dbd9aa888f430c0bd0dfb))
* **frontend:** Add server-side API proxy and fix SSR compatibility ([f562eba](https://github.com/Xevion/Banner/commit/f562ebaa5306a8253014895f025f052cc2a30700))
* **frontend:** Compress campus filter groups into compact availability URL param ([a2e9eb6](https://github.com/Xevion/Banner/commit/a2e9eb6cc77728cc7144e5e31c79c135b442467d))
* **frontend:** Redesign health and admin status pages ([843eacc](https://github.com/Xevion/Banner/commit/843eacc1533736bee217d58bd7b735060f24021d))
* Handle overenrolled sections in seat availability display ([5f699cf](https://github.com/Xevion/Banner/commit/5f699cf3689efa25986e68140e4127005b92c9dc))
* **infra:** Add database backup script with R2 upload and Docker support ([26b3349](https://github.com/Xevion/Banner/commit/26b3349f4623d72479d2fc871f8443cf27c96808))
* Integrate BlueBook scraper into scheduler with admin trigger endpoint ([9173aa7](https://github.com/Xevion/Banner/commit/9173aa73eee3ffe206724ca0660bee177916818f))
* **log:** Enhance tracing formatter with syntax highlighting and smart field grouping ([8d58f8a](https://github.com/Xevion/Banner/commit/8d58f8a6c78cfeb8ae5eedcbd7a7614bcc9fd8c2))
* Replace LATERAL RMP subqueries with materialized view ([730e75b](https://github.com/Xevion/Banner/commit/730e75bda4ef460215a0da9273e44a540057fb4b))
* **scraper:** Per-term adaptive scheduling with archived term tiers ([1491370](https://github.com/Xevion/Banner/commit/1491370fa0c4ea3b093b2947cbea8a286796ad29))
* **scripts:** Harden db reset with dynamic prompt ([b793d82](https://github.com/Xevion/Banner/commit/b793d822ce70565a5f52578408324be240fd8f79))
* Slug-based instructor filtering with autocomplete ([07aa9e5](https://github.com/Xevion/Banner/commit/07aa9e51a16d83a42d34b66aefa15a178bc28c53))
* Store instructors without email addresses ([d23d554](https://github.com/Xevion/Banner/commit/d23d554017019ef6c98290cc6fc59b6156a2f178))
* **stream:** Add ScraperStats, ScraperTimeseries, and ScraperSubjects stream kinds ([f32b727](https://github.com/Xevion/Banner/commit/f32b72754c0ab36161b97df56e65b33d8a78e693))
* Support numeric ID and email-prefix lookup for instructor endpoints ([00649fa](https://github.com/Xevion/Banner/commit/00649fa2c256ad30459f1a5fe68177b540b5f1bf))
* Surface BlueBook ratings and composite scores across instructor views ([57185a8](https://github.com/Xevion/Banner/commit/57185a8f8fdb7626280c9bfd816acac854a3ec41))
* Surface instructor subjects, years, and email in bluebook link review ([6913126](https://github.com/Xevion/Banner/commit/6913126594288b0f228c16f076f6f9b389f0e743))
* Switch frontend from SPA (adapter-static) to SSR with Bun ([bc5e60c](https://github.com/Xevion/Banner/commit/bc5e60c3b54785c885970a5c9da90efe05fa3e52))
* **terms:** Add multi-term scraping support with database table and admin API ([f5d13b0](https://github.com/Xevion/Banner/commit/f5d13b072a45ba01e540e330b0555dfc5e66f5b8))
* **web:** Replace TraceLayer with ULID-based request ID middleware ([8d2a1cd](https://github.com/Xevion/Banner/commit/8d2a1cddf0eb204f9d53122ca096440aae465ea1))


### Bug Fixes

* **backend:** Change valid year range to start in 2001 ([e081ace](https://github.com/Xevion/Banner/commit/e081ace0b4cd57e7a0999c1ed251ba4e5f7871f1))
* **backend:** Make hours_week nullable to handle null API responses ([ae035ec](https://github.com/Xevion/Banner/commit/ae035ecfd8799229bdfe98ec9eab4c94fdebf800))
* **backend:** Make instructional_method option to support older semesters ([7d97cc9](https://github.com/Xevion/Banner/commit/7d97cc976e1f7871189cb7390976c05bb23ecb6e))
* **backend:** Make wait_capacity, wait_count, and meeting_type nullable for older courses ([5445ba8](https://github.com/Xevion/Banner/commit/5445ba8f81bb42751ecfdbd305ded5524a415dd5))
* **bluebook:** Tolerate mismatched subject prefixes and non-breaking spaces in course parsing ([22e85c1](https://github.com/Xevion/Banner/commit/22e85c11a37d4e7f90d17e75cf0b04eee2490833))
* **ci:** Update bytes dependency for security audit fix, add security audits to check script ([049938a](https://github.com/Xevion/Banner/commit/049938a0a670414ac3e30ac178473ea05777118a))
* **deps:** Force ajv update to satisfy security vulnerability ([4626df1](https://github.com/Xevion/Banner/commit/4626df152fcec8d6159bf068fb169bc193fb14d2))
* **deps:** Update time to 0.3.47 for RUSTSEC-2026-0009 ([1321d1e](https://github.com/Xevion/Banner/commit/1321d1e2d9f6de8d8646b41dde45441785a33610))
* **frontend:** Card slide animation null height issues ([e78a46c](https://github.com/Xevion/Banner/commit/e78a46c990cedd1bf49270f8298536d67b446e06))
* **frontend:** Include title in teaching history each-block key to prevent duplicates ([f2afa8a](https://github.com/Xevion/Banner/commit/f2afa8a6550d55889036f2775c173cca1b12ec1d))
* **frontend:** Use pointer cursor on instructor link only, ignore link clicks for row expansions ([4679320](https://github.com/Xevion/Banner/commit/4679320b7f3f10ee23b96bb731ed50ba61fbbfcc))
* **frontend:** Use proper theme variables for styling scrollbar handle ([516f567](https://github.com/Xevion/Banner/commit/516f567148c2be74223dba4301db9301c3c46048))
* Improve search autocomplete popover trigger and z-index handling ([bb5aa3d](https://github.com/Xevion/Banner/commit/bb5aa3d206eecc925c8d156a699348c4d2518b9c))
* **infra:** Copy .sqlx cache and set SQLX_OFFLINE in Dockerfile ([4014e64](https://github.com/Xevion/Banner/commit/4014e64a4c48eaf3afeae24779109d987efcb985))
* **infra:** Install mold, add nextest, timeouts, and bun ci to CI ([9b33205](https://github.com/Xevion/Banner/commit/9b332050bba2ac8b9ae330a2703f7156b440c33b))
* **scripts:** Guard against re-entrant cleanup and handle SIGINT exit cleanly ([7cd4650](https://github.com/Xevion/Banner/commit/7cd465052748a2b75567e59f99bc3628ec862d31))
* Widen credit hour columns from INTEGER to DOUBLE PRECISION ([3b5184c](https://github.com/Xevion/Banner/commit/3b5184cca63401e2b0cf997cc0ce91471692a00b))


### Performance Improvements

* **backend:** Stream schedule cache loading with SQL-side meeting extraction ([0ec9eb7](https://github.com/Xevion/Banner/commit/0ec9eb77cc8c1d280361f6fd78ef6d16b713834b))
* **db:** Drop redundant/unused indexes, add scrape job index ([6663a99](https://github.com/Xevion/Banner/commit/6663a997dc0ec4ae276d84b38c4ae08a5f5211c5))
* **scripts:** Speed up check script by limiting run build & test scope to lib tests ([83e25e5](https://github.com/Xevion/Banner/commit/83e25e50382b4c20322a7e9b32547e50770bfcc4))


### Code Refactoring

* **admin:** Extract shared UI components and composables from admin pages ([c36d7d0](https://github.com/Xevion/Banner/commit/c36d7d055b114b43bd68d516bef3b2b2e8f90323))
* **backend:** Extract SearchOptionsCache with typed storage and singleflight ([80abd1d](https://github.com/Xevion/Banner/commit/80abd1dd59de5c5ffccdeb63cd32933c4748a47a))
* **backend:** Migrate audit old_value/new_value from TEXT to JSONB ([7c8ab19](https://github.com/Xevion/Banner/commit/7c8ab19d2c57135a2bb4643ddf818c1b735ecd7e))
* Clean up test helpers, switch reqwest to rustls, use mold linker ([5947633](https://github.com/Xevion/Banner/commit/59476337e65bf17a863df773c18e8ba399764c95))
* Consistent createContext, page titles, error page, test colocation ([0be4293](https://github.com/Xevion/Banner/commit/0be42936a11b1fd595078a286567e002d9551af6))
* **db:** Restructure database layer with context and event emission ([5551dc3](https://github.com/Xevion/Banner/commit/5551dc381f07aa7371726c9fbd3d9b012226d13b))
* Extract reusable score display components from inline rating logic ([0c1134f](https://github.com/Xevion/Banner/commit/0c1134f8e25d4ad8c5547b123448e43c340497cc))
* Extract RMP data layer, add frontend composables and UI components ([30c5372](https://github.com/Xevion/Banner/commit/30c53721cc97949981b401fde839ba261463fe7e))
* **frontend:** Extract filter logic into registry-driven pure functions ([aad0858](https://github.com/Xevion/Banner/commit/aad08581e989e99fca1160770c77fd1b1b517eb7))
* **frontend:** Hoist searchOptionsCache to module scope ([001dc0b](https://github.com/Xevion/Banner/commit/001dc0bff0a3ce0d4c2f187070c3515b99f3bf52))
* **frontend:** Move search fetching into SvelteKit load function ([f40e648](https://github.com/Xevion/Banner/commit/f40e648f688aa8fc8d08e0370ca1cfb487989a45))
* **middleware:** Consolidate rate limiting into unified module ([a69ba89](https://github.com/Xevion/Banner/commit/a69ba8964c0653fa82acd9eba6db03a7caec59aa))
* Normalize RMP data with sanitized nullable ratings ([69ec375](https://github.com/Xevion/Banner/commit/69ec375fabf811c8ad458598bc646a655518ee19))
* **query:** Replace Duration with NaiveTime for course search time parameters ([6252777](https://github.com/Xevion/Banner/commit/6252777b2946e6a8a987e880b00e75932c8a7879))
* Remove banner comments across codebase ([6813473](https://github.com/Xevion/Banner/commit/68134733fc821779cd6dbb00dbfa0b486771e4b9))
* Reorganize module structure, reduce top-level modules ([77e23b6](https://github.com/Xevion/Banner/commit/77e23b615ebebeff1fcda0a42b7096ffa8bd0423))
* Replace boolean confidence flags with continuous confidence score ([97dc185](https://github.com/Xevion/Banner/commit/97dc185b04f94e50f3cd445d03e550573625b595))
* **scraper:** Improve parse error handling and log formatting ([39c0ad6](https://github.com/Xevion/Banner/commit/39c0ad6681a0349e45652a0eece474ff8c6f5d20))
* **scripts:** Extract shared targets, commands, and preflight into reusable modules ([ecca783](https://github.com/Xevion/Banner/commit/ecca78397d6fa99410b7a075ae19ff9102498af5))
* Simplify LazyRichTooltip to use native tooltip open state ([025c2c4](https://github.com/Xevion/Banner/commit/025c2c417aab1a21d5ebb7e74f0a5bd4cdb8a06f))
* **tracing:** Improve log messages across codebase, standardize duration formatting ([0f230e6](https://github.com/Xevion/Banner/commit/0f230e677b0922791952a795935bf33d56e03a90))
* Unify instructor rating type names and add Brief/Full source variants ([80912b1](https://github.com/Xevion/Banner/commit/80912b1ac6e12b8cfe90dc2961cf8e8ac9ee3f90))
* **web:** Consolidate admin pages into tabbed scraper interface ([6fb7d79](https://github.com/Xevion/Banner/commit/6fb7d7913ac108e59ee5f937608844b8e2ab1ac5))


### Miscellaneous

* Normalize codebase symbols ([4b80c42](https://github.com/Xevion/Banner/commit/4b80c42dc35ad919f12682cbea68866e5c1aaf83))
* Reformat files ([968f6f4](https://github.com/Xevion/Banner/commit/968f6f470fe8b9641092dcd25b8d1eb4b2bcacd4))
* Remove section banner comments and dividers ([5a7bf21](https://github.com/Xevion/Banner/commit/5a7bf214c142dc7094ba6f096ce5fb7611729970))
* **scripts:** Add vite build to frontend check script ([ce10373](https://github.com/Xevion/Banner/commit/ce1037355f8eb51efd48992969dc2997a11e4edc))

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
