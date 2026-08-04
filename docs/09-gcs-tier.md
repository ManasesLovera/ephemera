# 09 — Google Cloud Storage tier, and how to set it up

## The idea

A fourth tier: **Google Cloud Storage**, reached the same way as the database — an
explicit "Save to cloud" button on any RAM or disk file card. It sits at the far end of
the hierarchy the app teaches: RAM (private, volatile, tiny) → disk (private, durable,
small) → database (structured, still local) → **cloud bucket (durable, remote, no
practical capacity ceiling)**. The cap for the in-app meter is a UI convention, not a
real GCS limit — flagged as an open question below.

## Current auth state on this machine — verified 2026-08-04

Checked before writing this doc, so the next session does not have to re-verify:

| Check | Result |
| --- | --- |
| `gcloud` SDK | 565.0.0, installed |
| Active account | `manaseslovera07@gmail.com` |
| Application Default Credentials (ADC) | present at `~/.config/gcloud/application_default_credentials.json`, token valid |
| Billing account | `Mi cuenta de facturación` (`014994-F214CD-819D45`), **open** |
| Target project | **`alterna-489722`** — billing enabled, confirmed |
| Existing buckets on that project | none |

Because ADC is already present and valid, **no browser login step is required** to get
started — `gcloud auth application-default login` has already been run in a prior
session. The remaining setup is: enable the API, create the bucket, and create a scoped
service account for the *app* to use (distinct from your personal login).

## Why a service account, not your personal ADC

Your personal ADC is tied to your Google account and has whatever access it has —
usually broad. The Tauri app should authenticate as its own identity with the minimum
permission it needs: read/write objects in exactly one bucket, nothing else in the
project. That is a **service account** scoped to that bucket only, with a downloaded
JSON key the app loads explicitly, kept out of the docs and out of git.

## Setup guide

Every command below targets `alterna-489722` explicitly via `--project`, so it never
touches your active gcloud config (`task-manager-493408`) or any other project.

### 1. Enable the Cloud Storage API

```bash
gcloud services enable storage.googleapis.com --project=alterna-489722
```

### 2. Create the bucket

Bucket names are globally unique across all of GCS, so it needs a distinguishing suffix.
Regional (not multi-region) keeps cost near zero for a demo-sized bucket.

```bash
gcloud storage buckets create gs://ephemera-vault-alterna \
  --project=alterna-489722 \
  --location=us-central1 \
  --uniform-bucket-level-access
```

If that name is taken (bucket names are global across *all* GCP customers), append a
random suffix, e.g. `ephemera-vault-alterna-mlovera`.

`--uniform-bucket-level-access` disables legacy per-object ACLs in favour of IAM-only
permissions — simpler to reason about and the current best practice.

### 3. Create a scoped service account

```bash
gcloud iam service-accounts create ephemera-app \
  --project=alterna-489722 \
  --display-name="Ephemera desktop app"
```

### 4. Grant it access to the bucket only — not the project

```bash
gcloud storage buckets add-iam-policy-binding gs://ephemera-vault-alterna \
  --member="serviceAccount:ephemera-app@alterna-489722.iam.gserviceaccount.com" \
  --role="roles/storage.objectAdmin"
```

`roles/storage.objectAdmin` on the *bucket* (not the project-level IAM) grants
read/write/delete on objects in that one bucket only — it cannot list or touch any other
bucket in the project. This is the scoping that matters.

### 5. Create and download a key

```bash
gcloud iam service-accounts keys create ~/dev/ephemera/src-tauri/gcs-key.json \
  --iam-account=ephemera-app@alterna-489722.iam.gserviceaccount.com
```

> [!warning]
> This file is a **credential** — anyone who has it can read/write the bucket. It must
> never be committed. Add to `.gitignore` immediately (see below), and treat it like a
> password: don't paste it into chat, a doc, or a screenshot.

### 6. Verify

```bash
gcloud storage ls gs://ephemera-vault-alterna --project=alterna-489722
GOOGLE_APPLICATION_CREDENTIALS=~/dev/ephemera/src-tauri/gcs-key.json \
  gcloud storage cp <(echo test) gs://ephemera-vault-alterna/smoke-test.txt
gcloud storage rm gs://ephemera-vault-alterna/smoke-test.txt
```

### `.gitignore` addition

```gitignore
# GCS credentials — never commit
src-tauri/gcs-key.json
src-tauri/.env
```

## What I have not yet done

Per your instruction to check with you before creating billable resources, **none of
steps 1–5 have been run yet**. Everything above is copy-pasteable and I can run it now,
step by step with your confirmation on each, or you can run it yourself — either works.
Cost note: this bucket, at demo data sizes (tens of MB), sits well inside GCS's free
tier (5 GB-months storage, 5,000 Class A ops/month); expected cost is effectively $0.

## Rust integration

| Crate | Purpose |
| --- | --- |
| `google-cloud-storage` | Async GCS client from the `google-cloud-rust` workspace; reads a service-account JSON key or ADC directly, no hand-rolled OAuth |
| `tokio` | Already present via Tauri |

```rust
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::upload::{UploadObjectRequest, UploadType, Media};

pub struct CloudStore {
    client: Client,
    bucket: String,
}

impl CloudStore {
    pub async fn connect(key_path: &Path, bucket: String) -> Result<Self, AppError> {
        std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", key_path);
        let config = ClientConfig::default().with_auth().await?;
        Ok(Self { client: Client::new(config), bucket })
    }

    pub async fn upload(&self, object_name: &str, bytes: Arc<[u8]>, mime: &str) -> Result<(), AppError> {
        let media = Media::new(object_name.to_string());
        // use the streaming/chunked upload variant for anything beyond a few MB —
        // same chunk-at-a-time principle as the disk streaming path in docs/07
        self.client.upload_object(
            &UploadObjectRequest { bucket: self.bucket.clone(), ..Default::default() },
            bytes.to_vec(), // or a streamed body for large files
            &UploadType::Simple(media),
        ).await?;
        Ok(())
    }
}
```

> [!note]
> Verify the exact `google-cloud-storage` crate API at implementation time — it is a
> fast-moving community crate and method signatures drift between versions. The
> streaming-upload variant (vs. the simple in-memory `Vec<u8>` shown above) is the one
> that matters for consistency with the "never hold the whole file unnecessarily"
> principle from [`07-streaming.md`](07-streaming.md); confirm its exact name in the
> version pinned in `Cargo.toml`.

Getting a usable object listing/size for the meter uses `client.list_objects` summed
client-side, or `bucket.get()` for aggregate metadata if the crate exposes it.

## IPC additions

| Command | Args | Returns | Notes |
| --- | --- | --- | --- |
| `save_to_cloud` | `id`, source (`ram` \| `disk`), `Channel` | `CloudFile` | Streams; reports upload throughput over the network, not disk — a new, slower comparison point |
| `list_cloud` | — | `Vec<CloudFile>` | |
| `delete_from_cloud` | `id` | `()` | |
| `get_cloud_status` | — | `CloudStatus { connected, bytes_used, object_count }` | Drives an offline/misconfigured banner, same pattern as the DB tier |

No `load_from_cloud` to RAM — same one-way rule as the database tier.

## UI

Same compact-panel treatment as the database tier (see
[`08-database-tier.md`](08-database-tier.md)): a "Save to cloud" button on RAM and disk
file cards, a segmented meter for the bucket, an offline/misconfigured state if the key
file or network is unavailable. Upload throughput to GCS is the slowest of every tier
measured so far and belongs in the extended throughput comparison chart (see
[`03-ui-and-visualization.md`](03-ui-and-visualization.md)) — network latency is a real,
felt number here, no throttle needed.

## Open questions specific to this tier

Tracked in full in [`06-open-questions.md`](06-open-questions.md); summarised here:

- **What cap should the GCS meter show?** GCS has no practical capacity limit at this
  scale — the cap exists only so the meter means something. Recommend reusing the DB
  tier's 100 MB as a default, configurable, with a note in the UI that this is a
  self-imposed demo limit, not a GCS limit — itself a teaching point (the hierarchy's
  last tier is the one where "the limit" stops being about capacity and starts being
  about cost/policy).
- **Region choice** — `us-central1` is a placeholder; pick based on where you are, or
  leave as-is since latency differences at this data volume are not the point.
