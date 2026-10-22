#!/bin/sh
# Update the Homebrew tap formula for a released tag.
#
# Usage:
#   scripts/update-tap.sh v0.4.0            # verify, update, commit, push
#   scripts/update-tap.sh v0.4.0 --dry-run  # verify and report without pushing
#
# Prerequisites: the tag must exist on GitHub (the formula builds from the
# source tarball, so no release artifacts are required), and git must be able
# to push to armaniacs/homebrew-sasurahime (gh auth login covers https).
set -eu

REPO="armaniacs/sasurahime"
TAP_REPO="armaniacs/homebrew-sasurahime"
VERSION="${1:?usage: scripts/update-tap.sh v0.4.0 [--dry-run]}"
DRY_RUN="${2:-}"

case "$VERSION" in
  v*) ;;
  *) echo "error: tag must start with v (got: $VERSION)" >&2; exit 1 ;;
esac

# 1. The tag must exist on GitHub: the formula reads the source tarball.
if ! gh api "repos/$REPO/git/ref/tags/$VERSION" >/dev/null 2>&1; then
  echo "error: tag $VERSION does not exist on GitHub. Push it first:" >&2
  echo "  git tag $VERSION && git push origin $VERSION" >&2
  exit 1
fi

# 2. sha256 of the source tarball the formula points at.
TARBALL="https://github.com/$REPO/archive/refs/tags/$VERSION.tar.gz"
SHA=$(curl -sLf "$TARBALL" | shasum -a 256 | cut -d' ' -f1)
URL="https://github.com/$REPO/archive/refs/tags/$VERSION.tar.gz"
echo "formula would point at: $URL"
echo "formula sha256:         $SHA"

# 3. Update the formula in a scratch clone of the tap.
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
git clone -q "https://github.com/$TAP_REPO" "$TMP"
FORMULA="$TMP/Formula/sasurahime.rb"
[ -f "$FORMULA" ] || { echo "error: $FORMULA not found in the tap" >&2; exit 1; }

sed -i '' \
  -e "s|url \"https://github.com/$REPO/archive/refs/tags/.*\"|url \"$URL\"|" \
  -e "s|sha256 \".*\"|sha256 \"$SHA\"|" \
  "$FORMULA"

echo "--- resulting formula (head) ---"
head -n 8 "$FORMULA"
echo "--------------------------------"

if git -C "$TMP" diff --quiet; then
  echo "tap formula already points at $VERSION: nothing to do."
  exit 0
fi

if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "dry-run: the formula would be updated as shown above (nothing pushed)."
  exit 0
fi

git -C "$TMP" -c user.name="armaniacs" -c user.email="armaniacs@users.noreply.github.com" \
  commit -am "sasurahime $VERSION"
git -C "$TMP" push -q origin HEAD
echo "tap updated: $TAP_REPO now serves $VERSION (brew upgrade to verify)."
