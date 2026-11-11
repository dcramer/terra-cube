# Surface materials

Photographed materials for the land's blocks, from [ambientCG](https://ambientcg.com) (CC0, no
attribution required). Each is `NAME.jpg` (colour) and `NAME_n.jpg` (normal map, OpenGL
convention), 512 px, shrunk from the 1K JPG set. `NAME` is a grain in
`crates/cube_voxel/src/texture.rs`, lower case; a grain with no file here is made in code.

| File | ambientCG asset |
|---|---|
| grass | Grass004 |
| soil | Ground048 |
| rock | Rock058 |
| strata | Rock030 |
| sand | Ground054 |
| snow | Snow010A |
| pebbles | Gravel023 |
| bark | Bark012 |
| endgrain | TreeEnd003 |
| litter | Ground106 |
| moss | Moss002 |
| leaves | ScatteredLeaves008 (fallen leaves, tinted green by the block: no scan of a crown was found) |
| needles | Ground077 (a floor of pine needles, tinted likewise) |

To add or swap one: download `ASSET_1K-JPG.zip`, take its `_Color` and `_NormalGL` images,
resize to 512, save under the grain's name. The block's own colour still tints it.
