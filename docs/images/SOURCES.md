# Image sources

| File | Content | Source / licence |
|---|---|---|
| `pixelwright-demo.jpg` | Pixelwright UI screenshot (rendered offscreen with `cargo run -p pixelwright-ui-egui --example snapshot`) editing *The Great Wave off Kanagawa*, Katsushika Hokusai, c. 1831 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Tsunami_by_hokusai_19th_century.jpg). UI: this project. |
| `pixelwright-layer-styles.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *Earthrise*, William Anders, Apollo 8, 24 December 1968 (NASA photograph) | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:NASA-Apollo8-Dec24-Earthrise.jpg). UI: this project. |
| `pixelwright-masks.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *Girl with a Pearl Earring*, Johannes Vermeer, c. 1665 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:1665_Girl_with_a_Pearl_Earring.jpg). UI: this project. |
| `pixelwright-vector.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *Water Lilies*, Claude Monet, 1906 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Claude_Monet_-_Water_Lilies_-_1906,_Ryerson.jpg). UI: this project. |
| `pixelwright-filters.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *The Starry Night*, Vincent van Gogh, 1889 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg). UI: this project. |
| `pixelwright-adjustments.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *Impression, Sunrise*, Claude Monet, 1872 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Monet_-_Impression,_Sunrise.jpg). UI: this project. |
| `pixelwright-transform.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *The Tetons and the Snake River*, Ansel Adams, 1942 (U.S. National Archives) | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Adams_The_Tetons_and_the_Snake_River.jpg). UI: this project. |
| `pixelwright-type.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *Among the Sierra Nevada, California*, Albert Bierstadt, 1868 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Albert_Bierstadt_-_Among_the_Sierra_Nevada,_California_-_Google_Art_Project.jpg). UI: this project. |
| `pixelwright-export-light.jpg` | Pixelwright UI screenshot (offscreen snapshot) with *The Kiss*, Gustav Klimt, 1907–1908 | Artwork: public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Gustav_Klimt_016.jpg). UI: this project. |

Screenshots must use famous public-domain artwork (or content made in the app), never personal photos.

# Web build samples

`apps/pixelwright-web/samples/{great-wave,starry-night,tetons,pearl-earring}.jpg` are the same public-domain artworks as the screenshots above (Hokusai, Van Gogh, Adams, Vermeer), downloaded from the Wikimedia Commons pages linked in the first table and resized to 1500 px wide (JPEG quality 84). They are offered on the start screen of the browser build and by `?open=samples/<name>.jpg`.

# Other bundled data

| File | Content | Source / licence |
|---|---|---|
| `assets/dict/en_US-scowl-50.txt.gz` | English word list (72,403 words, American spelling) for Edit › Check Spelling | [SCOWL](http://wordlist.aspell.net/) 2020.12.07, size 50, by Kevin Atkinson: permissive licence (use, copy, modify, distribute and sell, keeping the notice); full text and build recipe in `assets/dict/LICENSE-SCOWL.txt`. |
