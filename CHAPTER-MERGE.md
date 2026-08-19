* Make sure to have a `cleanups/` directory in the current
  directory with `git worktree`.

1. git checkout -b <chapter>
2. cp -a cleanups/<chapter>/* .
3. re-checkout files that should not be changed:
   Cargo.lock Cargo.toml Makefile
4. clean up README
5. gid add -p src
6. git commit -m 
7. git push
