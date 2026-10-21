#!/usr/bin/env bash
# Copies the stock Solomon Dark art the browser client loads at runtime into
# web/assets/stock. Normal builds use these tracked, hash-verified inputs.
# This reference-import tool is only needed when intentionally refreshing the art.
#
# Usage: scripts/sync-stock-assets.sh            (uses ../SolomonDarkAbandonware)
#        SOLOMON_STOCK_DIR=/path/to/stock scripts/sync-stock-assets.sh
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
stock="${SOLOMON_STOCK_DIR:-$root/../SolomonDarkAbandonware}/images"
out="$root/web/assets/stock"
website="${SOLOMON_WEBSITE_DIR:-$root/../website-rust-current}/frontend/src/assets/game"
mkdir -p "$out"
cp "${SOLOMON_WEBSITE_DIR:-$root/../website-rust-current}/frontend/public/favicon.png" "$out/favicon.png"
for name in Clothes College UI Skills Inventory LevelPicker Create ControlPanel BadGuys DeadHawg Title Library Golem Demon Faculty Heartmonger Unholy Bonedit; do
  cp "$stock/$name.bundle" "$out/$name.bundle"
  cp "$stock/$name.png" "$out/$name.png"
done
# The asset-reader regression checks retain these additional original bundles;
# their browser room/actor views use the stock-derived composites below.
for name in Memoratorium Office Solomon Storage; do
  cp "$stock/$name.bundle" "$out/$name.bundle"
done
# Font glyphs are painted directly from the stock mask atlas; their exact metrics live
# in sim::native_ui_text, not in the sprite-only bundle parser.
cp "$stock/Fonts.png" "$out/Fonts.png"
cp "$website/dark-cloud/leather.png" "$out/settings-leather.png"
# ControlPanel also stores font metrics; the shared sprite parser consumes the manifest's
# exact sprite-only projection, rather than interpreting glyph metrics as point counts.
python3 "$root/scripts/sync-ui-sprite-bundle.py" \
  "$website/native-ui-assets.json" ControlPanel "$out/ControlPanel.bundle"
# Reuse the Website's stock-derived composites. The unselected robe sheet already preserves
# the native f32 staff socket transform, and the front/back sheets preserve hand pass depth.
for name in \
  player-character-unselected-attachment-back \
  player-character-unselected-attachment-front \
  player-character-unselected-robe-attachment \
  hub-hud-bar-red hub-hud-bar-blue \
  hub-teacher-frames hub-teacher-rune \
  hub-teacher-burst-core hub-teacher-burst-column hub-teacher-burst-flare \
  hub-teacher-burst-frames; do
  cp "$website/$name.png" "$out/$name.png"
done
# The Website's retained stock-derived room layers preserve its registered composition,
# trimmed portrait cells and room foregrounds without duplicating the extraction tools.
cp "$website/boneyard/textures/greenplasma.png" "$out/enemy-green-plasma.png"
for name in anim-solomon-dig anim-solomon-encounter solomon-flydirt; do
  cp "$website/$name.png" "$out/$name.png"
done
for name in game-over-game game-over-over anim-solomon-riff; do
  cp "$website/$name.png" "$out/$name.png"
done
for name in \
  mortuary-background mortuary-paintings mortuary-flame mortuary-memorial-glow memorator \
  library-background library-props library-foreground library-flame librarian librarian-frames dowser \
  storeroom-background storeroom-props storeroom-foreground storeroom-flame \
  office-background office-prop office-foreground office-flame arch-desk arch-chancellor; do
  cp "$website/hub-room-$name.png" "$out/hub-room-$name.png"
done
# The authored death body, dyed garment and thrown weapon sheets preserve their
# native cell registration; the view composes the same independent tint passes.
for image in "$website"/player-character-death-*.png; do
  cp "$image" "$out/"
done
for name in skull-white frame-gold icon-bag icon-wave icon-infinity hub-hud-key-backing; do
  cp "$website/$name.png" "$out/$name.png"
done
# The Website retains these stock title/Create crops and the Darker logo composite.
# They are build inputs; the running Rust client does not read Website.
mkdir -p "$out/menu"
for image in "$website"/main-menu-*.png "$website"/menu-solomon-*.png \
  "$website"/create-hand-*.png "$website"/create-text-name-*.png \
  "$website/logo-solomon-dark.png"; do
  cp "$image" "$out/menu/"
done
cp "$website/../magenames.json" "$out/menu/wizard-names.json"
cp "$website/hall-of-fame-background.png" "$out/menu/hall-of-fame-background.png"
cp "$website/match-loading-background.png" "$out/menu/match-loading-background.png"
python3 "$root/scripts/sync-player-portrait-assets.py"
# BadGuys record 67 is the 25x25 player shadow (atlas x 405, y 1207). The world
# renderer uploads this small crop; the Create DOM view reads the full VFX atlas.
sips -s format png --cropToHeightWidth 25 25 --cropOffset 1207 405 \
  "$stock/BadGuys.png" --out "$out/player-shadow.png" >/dev/null
# Exact stock-derived Clothes cells preserve their native framed sampler bounds.
for name in player-harden-ice player-mindblast-ring; do
  cp "$website/$name.png" "$out/$name.png"
done
cp "$website/boneyard/textures/etherplane.png" "$out/etherplane.png"
ls -l "$out"

# Existing stock audio bindings include all combat/service/narration cues. Keep their
# source paths so the Rust catalog and standalone artifact require no JS asset runtime.
mkdir -p "$out/audio"
cp -R "$website/audio/." "$out/audio/"
cp "$website/../music/death.mp3" "$out/audio/music/death.mp3"

# Reuse the JS port's installed font faces for its native browser chat/card surfaces.
fontsource="${SOLOMON_WEBSITE_DIR:-$root/../website-rust-current}/frontend/node_modules/@fontsource"
mkdir -p "$out/fonts"
for spec in alegreya:400:normal alegreya:500:normal alegreya:700:normal alegreya:400:italic cinzel:500:normal cinzel:700:normal cinzel:800:normal jetbrains-mono:400:normal jetbrains-mono:600:normal; do
  IFS=: read -r family weight posture <<< "$spec"
  cp "$fontsource/$family/files/$family-latin-$weight-$posture.woff2" "$out/fonts/"
done
for family in alegreya cinzel jetbrains-mono; do
  cp "$fontsource/$family/LICENSE" "$out/fonts/$family-LICENSE.txt"
done

python3 "$root/scripts/verify-client-assets.py" --record
