# Frontend port guide (Phase 4)

The Vue app becomes a standalone Vite SPA. Pages and components stay; what
changes is where their props come from (the API instead of Inertia) and how
they send changes (the generated client instead of axios and Inertia
forms). The goal is no visible or behavioral change, except the upload
widget and the UI of removed features.

## 1. Pieces

| File | Role |
|---|---|
| `index.html`, `resources/js/app.js`, `App.vue` | Entry. `App.vue` renders the route's page with the props its loader returned, keyed by URL (pages remount on every visit, as with Inertia). |
| `router/routes.js` | Every page: Laravel's URL and route name, the page component, its loader. |
| `router/index.js` | Guards (login, email verification, guest pages, password confirmation, error page) and `route(name, params)` (Ziggy's `route()`, for SPA pages only; also a global in templates). `visit(url)` replaces `router.visit`. |
| `router/Link.vue`, `router/Head.vue` | Replace Inertia's `<Link>` and `<Head>`. |
| `lib/session.js` | `session.instance`, `session.user` (`CurrentUser` with `preferences`), `session.vaults` (`BootstrapVault[]`), fetched once per visit. `userName()`, `localeIetf()`. |
| `lib/form.js` | `useForm()` replacing Inertia's: fields + `errors`, `processing`, `recentlySuccessful`, `reset`, `clearErrors`, `data`, and `submit(send, {onSuccess, onError, onFinish})`. `pageErrors` feeds `Components/ValidationErrors.vue`. |
| `lib/format.js` | DateHelper (`formatDate`, `formatUserDate`, `isoFormat`, …), NameHelper (`contactName`), `label()` for seeded names, `sortByCollator`. |
| `lib/helpers.js` | Other `app/Helpers` ports: money, distance, file size, map links, important/reminder dates, slice date range, `avatar()`, `contactCard()`, `contactInformation()`, `groupsSummary()`. |
| `lib/help.js` | `helpUrl`, `helpLinks` (`$page.props.help_links` is now the global `helpLinks`). |
| `api/client.js` (generated) | One function per `operationId`: `op(params, body)`; `params` holds path and query parameters by name, `body` the JSON body or a `FormData`. `urls.<op>(params)` builds the URL of binary GETs (avatars, file content, vCard) for `<img src>` and downloads. |
| `api/runtime.js` | `ApiError` (`status`, `code`, `errors` as `{field: [messages]}`, and `response.data` = `{message, errors}` so existing `.catch((e) => (this.form.errors = e.response.data))` blocks keep working). CSRF handled here. |
| `loaders/<area>.js` | One loader per route: fetches what the page needs and returns its props. |
| `adapters/<area>.js` | Ports of the ViewHelpers: API objects → the props the page already expects. |
| `adapters/paginator.js` | `PaginatorHelper::getData` from a list's `meta` (for `Components/Pagination.vue`). |

`yarn api:draft` rebuilds `docs/rewrite/openapi.yaml` from the fragments and
regenerates `resources/js/api/*` from it. (At the end, `yarn api:generate`
generates the same files from the Rust server's spec.)

## 2. Porting a page

Worked example: `Settings/Personalize/Genders/Index.vue`, its loader
`personalize.genders` and adapter `adapters/personalize.js#gender`.

1. Find the page's route in `docs/rewrite/inventory.md` §2 (controller,
   ViewHelpers). Read the controller action and the ViewHelper methods that
   build the props, and the page and the components it renders.
2. Write the loader in `loaders/<area>.js`, replacing its `notPorted`
   stub. It receives the route (`to.params`, `to.query`, `to.meta`) and
   returns the props: `{ layoutData: layoutData(vaultId), data: … }` or
   whatever the controller passed. Call the API through `api/client.js`
   only; run independent calls in parallel (`Promise.all`). Use
   `currentSession()`/`session` for the user, preferences and vaults.
   Return `{ redirect: … }` for actions that redirected (e.g. `post.store`
   creates a post and goes to its edit page).
3. Write one adapter per ViewHelper `dto*`/`data` method in
   `adapters/<area>.js`, named after it (`dtoGender` → `gender`), with a
   doc comment naming the PHP method. Keep every field the page or its
   components read, with the same name and shape. Values the server
   formatted are formatted here with `lib/format.js`/`lib/helpers.js`;
   values the server computes (ages, streaks, next reminder dates, report
   totals, `can_upload_file`) come from the API.
4. `url` fields: page links become `route('<name>', {…})` (SPA routes keep
   Laravel's names and parameters). URLs that pointed at JSON or action
   routes are dropped from the props; the components call the client
   instead. Keep the ids a component needs to make those calls (or read
   them from `this.$route.params`).
5. Components: replace each `axios.<method>(url, data)` /
   `form.post(route(…))` / `router.post|delete(…)` with the client
   function of the matching operation, sending the fields of the spec's
   request schema (not the whole form object: forms carry `errors` and
   extras). Replace `response.data.data` with the adapter applied to the
   result. Keep the flashes, local list updates and error handling as
   they are. `router.visit(response.data.data)` (a URL the server
   returned) becomes `this.$router.push(route(…))` or `visit(route(…))`.
6. Imports: `Link` from `@/router/Link.vue`, `Head` from
   `@/router/Head.vue`, `useForm` from `@/lib/form`; `usePage` and
   `$page` are gone (`session`, `helpLinks`, `$route.meta.component`).

Uploads (`Components/Uploadcare.vue` users) go through a `FileUpload`
component posting `FormData` with one `file` part to the upload operation
(api-design.md §10); images use `urls.getVaultFileContent({vault_id,
file_id, variant: 'thumbnail'})` for squares up to 300 px and `original`
otherwise.

## 3. Rules

1. Every request goes through `resources/js/api/client.js`. No `axios`,
   `fetch`, `XMLHttpRequest`, Inertia or Ziggy anywhere else. `<a href>`
   and `<img src>` to API URLs are built with `urls.*` only.
2. Pages keep their templates. Change template code only where it used a
   removed thing (`route()` of a non-page route, `$page`, removed
   features) or a prop that can't keep its shape.
3. Removed features lose their UI: OAuth providers, WebAuthn, Telegram,
   Uploadcare, Mapbox static maps, Sentry, the customers.monicahq.com line.
   Nothing else is cut. If a page needs data the spec doesn't provide,
   that is a spec gap (rule 5), not a reason to drop it.
4. Don't edit `router/*`, `lib/*`, `api/*`, `App.vue`, `app.js`,
   `Layouts/*` or other areas' loaders/adapters. Helpers you need that
   aren't in `lib/` go in your adapters file; list them in the report.
5. Spec gaps: fix the fragment of your area in `docs/rewrite/api/` (field
   or parameter missing, an operation missing), keeping the conventions of
   api-design.md, then `yarn api:draft`. Operations already implemented in
   Rust are frozen (the list is in your brief); report gaps in those
   instead. List every spec change in the report.
6. `yarn lint` has no error and no new warning in your files; `yarn vite
   build` passes.
