# Misc

## To Keep In Mind

- attempts need dApp and wApp version
- backups/snapshots should be taken on cadence, as well as immediately before a deploy
- consider deployment strategy for services
  - in general, forcing backwards compatible bumps is only way forward
  - [ ] each service is deployed with redundancy (rollover)
  - clients and servers should have round-one version compatibility
    - forced client updates

### Examiner Dashboard

- ability to delete branches
- separate branch management from change requests
  - promoting dev -> stg -> prd should be its own thing
- use GitHub naming for familiarity
