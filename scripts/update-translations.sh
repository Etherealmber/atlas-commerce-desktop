#!/usr/bin/env bash
# Re-extracts the gettext strings from the Rust sources and merges them into
# the French catalog, preserving existing translations.
#
# Requires gettext (xgettext, msgmerge). The build itself needs no gettext:
# fastframe-i18n compiles the committed .po file directly.
set -euo pipefail

cd "$(dirname "$0")/.."

for tool in xgettext msgmerge msgattrib; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: $tool is required (install the gettext package)" >&2
        exit 1
    fi
done

# --language=Rust is only understood by GNU gettext 0.24 and newer; Ubuntu
# 24.04 still ships 0.21, so fail with an actionable message instead of a
# cryptic xgettext error.
xgettext_version=$(xgettext --version | awk 'NR==1 {for (i=1;i<=NF;i++) if ($i ~ /^[0-9]+\.[0-9]+/) {print $i; exit}}')
if ! awk -v v="$xgettext_version" 'BEGIN { split(v,a,"."); if ((a[1]+0) > 0 || (a[2]+0) >= 24) exit 0; exit 1 }'; then
    cat >&2 <<EOF
error: xgettext $xgettext_version is too old: Rust extraction (--language=Rust)
       requires GNU gettext >= 0.24. Supported installs:
         - macOS:  brew install gettext
         - Linux:  a distro packaging gettext >= 0.24, or the GNU tarball
                   (https://www.gnu.org/software/gettext/)
EOF
    exit 1
fi

catalog="crates/ui/assets/i18n/fr.po"
template="$(mktemp --suffix=.pot)"
trap 'rm -f "$template"' EXIT

# t!(locale, "string") — the msgid is the second macro argument.
# shellcheck disable=SC2046
xgettext \
    --language=Rust \
    --from-code=UTF-8 \
    --add-comments \
    --sort-output \
    --no-location \
    --keyword='t!:2' \
    --copyright-holder='loxdpsy/Etherealmber' \
    --package-name='atlas-commerce-desktop' \
    --package-version='0.1.0' \
    -o "$template" \
    $(find crates -name '*.rs' -not -path '*/target/*')

msgattrib --no-obsolete "$catalog" > "${catalog}.clean"
mv "${catalog}.clean" "$catalog"
msgmerge --update --sort-output --no-fuzzy-matching "$catalog" "$template"

echo "Updated $catalog — fill any empty msgstr entries, then rebuild."
