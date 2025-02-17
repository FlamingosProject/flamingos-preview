* Make sure to have a `cleanups/` directory in the current
  directory with `git worktree`.

0. git checkout -b <chapter>
1. cp -a cleanups/<chapter>/* .
2. re-checkout files that should not be changed:
   Cargo.lock Cargo.toml Makefile
3. gid add -p to get the rest of the stuff updated
4. git commit -m 
5. git push
