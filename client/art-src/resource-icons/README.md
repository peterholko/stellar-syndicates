# Resource icon sources (1254×1254 PNG, real alpha)

Source-of-truth for the six commodity/credit icons. NOT bundled (outside
`public/`). The UI loads downscaled 64px variants under
`public/art/ui_icons/resource/`.

Regenerate the variants (macOS `sips`, high-quality downscale):

    for f in ore alloys fuel provisions volatiles credits; do
      sips -s format png -Z 64 "icon_$f.png" \
        --out "../../public/art/ui_icons/resource/$f.png"
    done
