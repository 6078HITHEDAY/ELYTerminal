#!/bin/sh
# release-asset.sh FILE: puts FILE on the Gitea release of this version, a draft too, in place of one of its name.
set -eu
cd "$(dirname "$0")/.."
file=$1
[ -f "$file" ] || { echo "release-asset: no file $file" >&2; exit 1; }
: "${RELEASE_API:?release-asset: RELEASE_API names the repository API, https://HOST/api/v1/repos/OWNER/REPO}"
: "${RELEASE_TOKEN:?release-asset: RELEASE_TOKEN is a token that may edit the releases}"
version=$(cargo pkgid)
version=${version##*[#@]}
name=$(basename "$file")
auth="Authorization: token $RELEASE_TOKEN"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsS -H "$auth" "$RELEASE_API/releases?limit=50" > "$work/releases.json"
ids=$(python3 -c '
import json, sys
tag, name, path = sys.argv[1:]
found = [r for r in json.load(open(path)) if r["tag_name"] == tag]
if len(found) != 1:
    sys.exit(f"release-asset: {len(found)} releases tagged {tag}")
print(found[0]["id"], *(a["id"] for a in found[0]["assets"] if a["name"] == name))
' "v$version" "$name" "$work/releases.json")
set -- $ids
release=$1
shift
for old in "$@"; do
  curl -fsS -X DELETE -H "$auth" "$RELEASE_API/releases/$release/assets/$old"
  echo "release-asset: removed the old $name"
done
curl -fsS -H "$auth" -F "attachment=@$file" "$RELEASE_API/releases/$release/assets?name=$name" > /dev/null
echo "release-asset: $name on release $release, v$version"
