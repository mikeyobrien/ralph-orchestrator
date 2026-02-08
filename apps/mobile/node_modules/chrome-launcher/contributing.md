# Release Procedure

1. When good, do `npm publish`. it'll run the prepublish script (build & test)
   * complete publishing to npm. (we now have the tagged commit, too).
1. To complete the changelog: Do `git log --oneline` and then edit the resulting text to match the style in `changelog.md`. it's pretty manual. update that file. commit.
1. Push back up to origin.  Do `git push --tags` to ensure the tag is there.
1. Open up https://github.com/GoogleChrome/chrome-launcher/releases and _Draft a New Release_. Use the same changelog.md markdown.  Publish.
