# Ship modules and sized parts

Generated Nov 8 2026 with the built-in `image_gen` tool from
`docs/shipbuilding-art-prompts.md`: 17 module masters and 18 heavy/capital
masters. All are native 1254 × 1254 RGBA PNGs with genuine transparency.
Their measured silhouettes fill about 80–90% of the canvas.

The selected generated files were copied unchanged: no local crop, resize or
alpha processing. Composition corrections and the Capital Tempest Drive's
gantry-gap cleanup were made through image generation before selecting the
final files. Exact initial and correction prompts are in [prompts.json](prompts.json).
[generation-manifest.json](generation-manifest.json) records reference hashes,
generated source paths, final hashes, dimensions, bounds and transparency.

Derived and wired into the game on Nov 8 2026 ("Wiring" in
`docs/shipbuilding-art-prompts.md`).

## Modules

| Module | Master |
|---|---|
| Mass Driver | [mass_driver.png](mass_driver.png) |
| Torpedo Rack | [torpedo_rack.png](torpedo_rack.png) |
| Point-Defense Screen | [point_defense_screen.png](point_defense_screen.png) |
| Lance Array | [lance_array.png](lance_array.png) |
| Prismatic Lance | [prismatic_lance.png](prismatic_lance.png) |
| Reflective Plating | [reflective_plating.png](reflective_plating.png) |
| Whipple Armor | [whipple_armor.png](whipple_armor.png) |
| Extended Tanks | [extended_tanks.png](extended_tanks.png) |
| Recon Suite | [recon_suite.png](recon_suite.png) |
| Cargo Pods | [cargo_pods.png](cargo_pods.png) |
| Escort Datalink | [escort_datalink.png](escort_datalink.png) |
| Fuel Transfer Rig | [fuel_transfer_rig.png](fuel_transfer_rig.png) |
| Survey Booster | [survey_drive.png](survey_drive.png) |
| Nebula Spectrometer | [nebula_spectrometer.png](nebula_spectrometer.png) |
| Stasis Web | [stasis_web.png](stasis_web.png) |
| Warp Stabilizer | [warp_stabilizer.png](warp_stabilizer.png) |
| Interdiction Field | [interdiction_field.png](interdiction_field.png) |

The five weapon modules use the Nov 7 Pulse Laser as their style reference.
Prismatic Lance establishes the pearl-white material shared by Survey Booster
and Nebula Spectrometer. The Survey Booster keeps the filename `survey_drive`.

## Sized parts

Both sizes begin as direct edits of their light master. Heavy parts add a low
transport cradle and two lifting lugs; capital parts add an open gantry with
a narrow catwalk and handrail. Reactor and drive light masters are in
`../goods-2026-11-07/`; the Hull Sections light master is in
`../goods-2026-10-24/`.

| Light part | Heavy master | Capital master |
|---|---|---|
| Fission Reactor | [heavy_fission_reactor.png](heavy_fission_reactor.png) | [capital_fission_reactor.png](capital_fission_reactor.png) |
| Fusion Reactor | [heavy_fusion_reactor.png](heavy_fusion_reactor.png) | [capital_fusion_reactor.png](capital_fusion_reactor.png) |
| Advanced Fusion Reactor | [heavy_advanced_fusion_reactor.png](heavy_advanced_fusion_reactor.png) | [capital_advanced_fusion_reactor.png](capital_advanced_fusion_reactor.png) |
| Antimatter Reactor | [heavy_antimatter_reactor.png](heavy_antimatter_reactor.png) | [capital_antimatter_reactor.png](capital_antimatter_reactor.png) |
| Zephyr Drive | [heavy_zephyr_drive.png](heavy_zephyr_drive.png) | [capital_zephyr_drive.png](capital_zephyr_drive.png) |
| Gale Drive | [heavy_gale_drive.png](heavy_gale_drive.png) | [capital_gale_drive.png](capital_gale_drive.png) |
| Tempest Drive | [heavy_tempest_drive.png](heavy_tempest_drive.png) | [capital_tempest_drive.png](capital_tempest_drive.png) |
| Maelstrom Drive | [heavy_maelstrom_drive.png](heavy_maelstrom_drive.png) | [capital_maelstrom_drive.png](capital_maelstrom_drive.png) |
| Hull Sections | [heavy_hull_sections.png](heavy_hull_sections.png) | [capital_hull_sections.png](capital_hull_sections.png) |
