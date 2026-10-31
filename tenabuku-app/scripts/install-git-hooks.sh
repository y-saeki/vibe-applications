#!/bin/sh
# Installs git hooks that regenerate Tenabuku.xcodeproj with XcodeGen after a
# pull, a rebase or a branch switch, so a newly added file or a changed
# project.yml never goes unnoticed in Xcode. Run once per clone.
set -eu

marker="# tenabuku-app: regenerate Tenabuku.xcodeproj"
hooks_dir="$(git rev-parse --git-path hooks)"
mkdir -p "$hooks_dir"

for hook in post-merge post-checkout post-rewrite; do
  path="$hooks_dir/$hook"
  if [ -e "$path" ] && ! grep -qF "$marker" "$path"; then
    echo "$path already exists and was not installed by this script; left it as is." >&2
    echo "Add a call to xcodegen generate in tenabuku-app to it by hand if you want one." >&2
    exit 1
  fi
  cat > "$path" <<HOOK
#!/bin/sh
$marker (installed by tenabuku-app/scripts/install-git-hooks.sh)

# post-checkout: skip checkouts of single files (\$3 = 0).
# post-rewrite: only after a rebase, not after an amend.
case "\$(basename "\$0")" in
  post-checkout) [ "\${3:-1}" = 0 ] && exit 0 ;;
  post-rewrite) [ "\${1:-}" = rebase ] || exit 0 ;;
esac

dir="\$(git rev-parse --show-toplevel)/tenabuku-app"
[ -f "\$dir/project.yml" ] || exit 0

# Git clients outside a login shell may not have Homebrew on PATH.
PATH="\$PATH:/opt/homebrew/bin"
if ! command -v xcodegen >/dev/null 2>&1; then
  echo "tenabuku-app: xcodegen not found; Tenabuku.xcodeproj was not regenerated." >&2
  exit 0
fi

(cd "\$dir" && xcodegen generate --quiet) ||
  echo "tenabuku-app: xcodegen generate failed; run it by hand to see why." >&2
exit 0
HOOK
  chmod +x "$path"
  echo "Installed $path"
done
