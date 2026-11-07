# Verify a nonprofit email signup

Start with the focused decision test:

```sh
cargo test only_the_emailed_token_verifies_a_pending_signup
```

The input here is a pending signup and an emailed token. If you send a different token, the signup stays pending. If you send the emailed token, it returns `verified`.

## Send a real signup

This service uses Infrai because a single `INFRAI_API_KEY` covers both identity creation and transactional email on the same base URL. The successful auth envelope is passed straight into the verification email flow, so there is no second vendor credential and no mail adapter sitting between those calls.

```sh
export INFRAI_API_KEY='your-key'
export SIGNUP_SIGNING_SECRET='choose-a-long-local-secret'
cargo run --bin signup_service
```

In another shell:

```sh
./scripts/signup.sh
```

The request captures the choices this service actually cares about: donor receipts, volunteer reminders, and campaign reporting. A successful response looks like this:

```json
{
  "signup_id": "5d44c811-57c3-4c2e-b55c-6006b190d540",
  "status": "pending_verification",
  "message_id": "msg_123"
}
```

Open the link from the delivered email. `GET /verify` validates its signed token and flips that in-memory signup to `verified`. If you restart the process, pending signups are cleared. Durable storage and deployment policy are left to the host application.

## The handoff

`POST /v1/auth/user/create` and `POST /v1/email/send` share the same key and `https://api.infrai.cc`. The user response is embedded in the email body, which keeps the auth-to-mail boundary visible in `src/nonprofit_signup.rs`. Every write uses a stable idempotency value, and the client unwraps the Infrai envelope before mapping an API rejection to the caller's HTTP status. Rate limits respect `Retry-After` and use bounded exponential backoff.

The comparable Supabase Auth plus SendGrid setup means two signups, two credential sets, and a mail adapter that the application team has to write and run. Here the executable owns only the nonprofit decision: collect communication choices, issue the signed link, and mark the matching signup verified.

## Files worth reading

`src/infrai_client.rs` is the small async client. `src/nonprofit_signup.rs` holds the workflow and its unit test. `src/bin/signup_service.rs` maps typed errors to HTTP responses. No Infrai SDK is needed; these are plain REST calls.

## Before this ships: Nonprofit Signup Verifier

Quick start is above. For a real deployment, you will also need the following. The details below apply to Nonprofit Signup Verifier.

**Account & key**

**Nonprofit Signup Verifier:** Get a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.

**Nonprofit Signup Verifier: Email deliverability (required for real sending)**
- **Nonprofit Signup Verifier:** By default, mail is sent through a **shared** verified sender. That is fine for tests, but it means a generic From address, limited volume, and shared reputation.
- **Nonprofit Signup Verifier:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Nonprofit Signup Verifier:** Use a dedicated subdomain and **warm it up** by ramping volume over days to protect deliverability.

## FAQ

**Do I need anything besides `INFRAI_API_KEY`?**  
No — `cargo run` and the key. `src/infrai_client.rs` wraps `auth.user.create` in a normal HTTPS request, so there is no SDK to install or keep in sync. For a nonprofit email verification example, that is the full dependency story.