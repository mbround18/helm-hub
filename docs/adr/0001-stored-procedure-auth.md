# ADR 0001: Use stored procedures for register/login

**Status:** Accepted

## Context

The backend currently enforces PostgreSQL row-level security (RLS) on `users`.
That is good for protecting user data, but direct application queries for
`register` and `login` conflict with the policy because those requests happen
before a user session exists.

We need an auth flow that:

- keeps RLS enabled for `users`
- supports anonymous registration
- supports login without weakening the table policy
- avoids ad hoc policy exceptions that only exist for auth endpoints

## Decision

Use PostgreSQL stored procedures for the auth flow:

- `register_user(...)` inserts the new user row
- `login_user(...)` performs the credential lookup and returns the user record

Both procedures should run with controlled elevated privileges (`SECURITY
DEFINER`) and encapsulate the minimal database access needed for auth.

## Options considered

| Option | Pros | Cons |
| --- | --- | --- |
| Keep auth in the application and relax `users` RLS for signup/login cases | Simple to code in Rust; easy to debug in one layer | Weakens the RLS model; adds special-case policy logic; auth becomes a loophole in the DB boundary |
| Use stored procedures for register/login | Keeps RLS intact; narrows privileged access to a small DB surface; auth behavior lives next to the data it protects | More database code; harder to test than plain app queries; requires careful `SECURITY DEFINER` review |

## Consequences

- `users` can stay protected by RLS for normal application queries.
- Auth behavior becomes a small, explicit database API instead of a broad RLS
  exception.
- Future auth changes will need both Rust and SQL migration updates.
- The procedures must be reviewed carefully to avoid privilege escalation.

## Migration log

1. `2026-05-18-0002_auth_context_users_policy` — replaces the temporary
   `app.login_username` policy escape hatch with a narrow `app.auth_context`
   gate so only the auth procedures can touch `users` during bootstrap auth.
2. `2026-05-18-0003_create_register_user_function` — adds
   `auth.register_user(...)` so registration inserts happen through a single
   controlled database entrypoint.
3. `2026-05-18-0004_create_login_user_function` — adds
   `auth.login_user(...)` so the login lookup happens through a single
   controlled database entrypoint while password verification remains in Rust.
4. `2026-05-18-0006_create_oauth_accounts` — adds the provider identity map so
   GitHub/GitLab logins can resolve a stable external account to one local
   user row without relaxing RLS on `users`.
5. `2026-05-18-0007_create_oauth_upsert_function` — adds
   `auth.upsert_oauth_user(...)` so OAuth sign-ins can provision or resolve the
   local user through one controlled database entrypoint instead of direct Rust
   inserts.

## Notes

The current app-level login workaround should be treated as temporary until the
stored procedures replace it.
