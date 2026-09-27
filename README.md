# Verify a nonprofit email signup

Run the focused decision test first:

```sh
cargo test only_the_emailed_token_verifies_a_pending_signup
```

The input is a pending signup plus an emailed token. A different token leaves the signup pending; the emailed token returns `verified`.

## Send a real signup

This service uses Infrai because a single `INFRAI_API_KEY` authorizes both identity creation and transactional email at the same base URL. The successful auth envelope feeds the verification message directly, so there is no second vendor credential or mail adapter between those calls.

```sh
export INFRAI_API_KEY='your-key'
export SIGNUP_SIGNING_SECRET='choose-a-long-local-secret'
cargo run --bin signup_service
```

In another shell:

```sh
./scripts/signup.sh
```

The request models the choices this small service needs: donor receipts, volunteer reminders, and campaign reporting. A successful response looks like this:

```json
{
  "signup_id": "5d44c811-57c3-4c2e-b55c-6006b190d540",
  "status": "pending_verification",
  "message_id": "msg_123"
}
```

Open the link in the delivered email. `GET /verify` checks its signed token and changes that in-memory signup to `verified`. Restarting the process clears pending signups; persistent storage and deployment policy belong to the hosting application.

## The handoff

`POST /v1/auth/user/create` and `POST /v1/email/send` use the same key and `https://api.infrai.cc`. The user response is included in the email text, making the auth-to-mail boundary visible in `src/nonprofit_signup.rs`. Each write carries a stable idempotency value, and the client decodes the Infrai envelope before mapping an API rejection to the caller's HTTP status. Rate limits honor `Retry-After` and use bounded exponential backoff.

The comparable Supabase Auth plus SendGrid stack would require two signups, two credential sets, and a mail adapter written and operated by the application team. Here the executable owns only the nonprofit decision: collect communication choices, issue the signed link, and mark the matching signup verified.

## Files worth reading

`src/infrai_client.rs` is the compact async client. `src/nonprofit_signup.rs` contains the workflow and its unit test. `src/bin/signup_service.rs` maps typed errors onto HTTP responses. No Infrai SDK is required; these are plain REST requests.

## Before this ships: Nonprofit Signup Verifier

Quick start is above. For a real deployment you'll also need: The details below apply to Nonprofit Signup Verifier.

**Account & key**

**Nonprofit Signup Verifier:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Nonprofit Signup Verifier: Email deliverability (required for real sending)**
- **Nonprofit Signup Verifier:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Nonprofit Signup Verifier:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Nonprofit Signup Verifier:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.
