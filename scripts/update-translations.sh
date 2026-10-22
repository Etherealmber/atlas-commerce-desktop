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
