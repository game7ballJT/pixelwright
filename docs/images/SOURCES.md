# Image sources

Screenshots are rendered with `scripts/readme-shots.ps1` (the offscreen `snapshot` example) from the 1500 px copies of the artwork in `apps/pixelwright-web/samples/`.

| File | Content | Source / licence |
|---|---|---|
| `pixelwright-demo.jpg` | Pixelwright UI screenshot editing *The Great Wave off Kanagawa*, Katsushika Hokusai, c. 1831, with a type layer, a drop shadow, a Vibrance layer and a palette taken from the image | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Tsunami_by_hokusai_19th_century.jpg). UI: this project. |
| `pixelwright-pixel-art.jpg` | The same artwork after Filter > Pixelate > Pixel Art (6 px cells, 16 colours, ordered dither) | Artwork: public domain (same source). Result and UI: this project. |
| `pixelwright-swatches.jpg` | The Swatches panel with a palette extracted from *The Starry Night*, Vincent van Gogh, 1889 (a crop of a screenshot) | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg). UI: this project. |
| `pixelwright-retouch.jpg` | Frequency Separation and a Dodge & Burn layer on *Girl with a Pearl Earring*, Johannes Vermeer, c. 1665 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:1665_Girl_with_a_Pearl_Earring.jpg). UI: this project. |

Screenshots must use famous public-domain artwork (or content made in the app), never personal photos.

# Web build samples

`apps/pixelwright-web/samples/{great-wave,starry-night,tetons,pearl-earring}.jpg` are the same public-domain artworks as the screenshots above (Hokusai, Van Gogh, Adams, Vermeer), downloaded from the Wikimedia Commons pages linked in the first table (*The Tetons and the Snake River*, Ansel Adams, 1942, U.S. National Archives: [Commons](https://commons.wikimedia.org/wiki/File:Adams_The_Tetons_and_the_Snake_River.jpg)) and resized to 1500 px wide (JPEG quality 84). They are offered on the start screen of the browser build and by `?open=samples/<name>.jpg`.

# Other bundled data

| File | Content | Source / licence |
|---|---|---|
| `assets/dict/en_US-scowl-50.txt.gz` | English word list (72,403 words, American spelling) for Edit > Check Spelling | [SCOWL](http://wordlist.aspell.net/) 2020.12.07, size 50, by Kevin Atkinson: permissive licence (use, copy, modify, distribute and sell, keeping the notice); full text and build recipe in `assets/dict/LICENSE-SCOWL.txt`. |
