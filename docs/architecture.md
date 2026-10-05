## Architecture

```mermaid
graph LR
  subgraph candidate[Candidate devices]
    dApp["Desktop App<br/>[dApp]"]
    wApp["Web App<br/>[wApp]"]
  end

  aAPI["Auth API<br/>[aAPI]"]
  cAPI["Curriculum API<br/>[cAPI]"]
  cDb[("Curriculum Database<br/>[cDb]")]
  mDb[("Moderation Database<br/>[mDb]")]
  eDd["Examiner Dashboard<br/>[eDd]"]
  eDb[("Examiner Database<br/>[eDb]")]

  dApp <--> aAPI
  wApp <--> aAPI
  aAPI --> cAPI
  cAPI <--> cDb
  aAPI <--> mDb
  eDd <--> cAPI
  eDd <--> mDb
  eDd <--> eDb
  eDd --- cDbClone[("Authoring clone<br/>[Dolt]")]
  cDb -. fetch, driven by cAPI .-> cDbClone
  cDbClone -. clone and schema pull, read-only .-> cDb
```

| Application | Responsibility                                                                                                                                                                                                                    |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **dApp**    | Serves exam content to the candidate and observes the station. Tauri.                                                                                                                                                             |
| **wApp**    | Second candidate device, a web app so a spare PC can serve. Captures biometrics and station evidence. Replaces the mobile app the earlier design assumed.                                                                         |
| **aAPI**    | Owns candidate identity and the attempt path. The only application candidate devices reach. Reads content through cAPI; writes attempts and evidence to mDb. Rust, Axum, Tokio.                                                   |
| **cAPI**    | Sole holder of cDb credentials and sole writer of cDb. Fetches authored history from eDd's clone, fast-forwards `dev`, owns promotion, and serves redacted content to aAPI. Rust, Axum, Tokio.                                    |
| **cDb**     | Exam content and its whole history. Self-hosted Dolt. Must not lose content.                                                                                                                                                      |
| **mDb**     | Candidate identity, attempts, responses and evidence. All candidate data.                                                                                                                                                         |
| **eDd**     | Where examiners author, review, promote and moderate. Owns examiner identity, roles and scopes, and runs its own Dolt instance - the authoring clone - where all authoring happens. Astro, React, TypeScript, with a Rust server. |
| **eDb**     | Everything eDd owns that is not versioned curriculum content. PostgreSQL in first authoring sweep.                                                                                                                                |
