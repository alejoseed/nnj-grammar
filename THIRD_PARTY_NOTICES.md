# Third-Party Notices

## Hanabira Japanese Content

The generated grammar rules under `grammar/hanabira/` are derived from
[Hanabira Japanese Content](https://github.com/tristcoil/hanabira.org-japanese-content).
The source repository describes the content as Creative Commons and requires
users to link to [hanabira.org](https://hanabira.org/).

The upstream repository does not currently identify a specific Creative
Commons license variant. Verify the applicable terms with the upstream author
before redistributing the generated grammar data.

## UD Japanese-GSD Treebank

The gold-standard evaluation corpus under `data/ud-japanese-gsd/` is a clone of
[UD_Japanese-GSD](https://github.com/UniversalDependencies/UD_Japanese-GSD),
licensed under
[CC BY-SA 4.0](http://creativecommons.org/licenses/by-sa/4.0/legalcode)
(see `data/ud-japanese-gsd/LICENSE.txt`). It is used only as a test fixture for
the 文節 chunker (`chunker-eval`); no treebank content is embedded in the
shipped binaries.
