## What changed

## How to check

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `DATABASE_URL=postgres://arena:arena@127.0.0.1:5432/arena cargo test`
- [ ] `cd web && npm run lint && npm run build`
- [ ] `shellcheck scripts/connect.sh` if `scripts/connect.sh` changed

## Notes

No tokens, private keys, or filled-in `.env` files in the diff.
