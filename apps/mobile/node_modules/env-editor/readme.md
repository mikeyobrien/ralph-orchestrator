# env-editor

> Get metadata on the default editor or a specific editor

This module is used by [`open-editor`](https://github.com/sindresorhus/open-editor).


## Supported editors

- Sublime Text
- Atom
- Visual Studio Code
- WebStorm
- TextMate
- Vim
- NeoVim
- IntelliJ
- GNU nano
- GNU Emacs
- Android Studio


## Install

```
$ npm install env-editor
```


## Usage

```js
const {defaultEditor, getEditor, allEditors} = require('env-editor');

defaultEditor();
/*
{
	id: 'atom',
	name: 'Atom',
	binary: 'atom',
	isTerminalEditor: false,
	paths: [
		'/Applications/Atom.app/Contents/Resources/app/atom.sh'
	],
	keywords: []
}
*/

getEditor('sublime');
/*
{
	id: 'sublime',
	name: 'Sublime Text',
	binary: 'subl',
	isTerminalEditor: false,
	paths: [
		'/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl',
		'/Applications/Sublime Text 2.app/Contents/SharedSupport/bin/subl'
	],
	keywords: []
}
*/

allEditors();
/*
[
	{
		id: 'atom',
		…
	},
	{
		id: 'sublime,
		…
	},
	…
]
*/
```


## API

### defaultEditor()

Returns metadata on the default editor.

The user is expected to have the `$EDITOR` environment variable set, and if not, a user-friendly error is thrown.

### getEditor(editor)

Returns metadata on the specified editor.

#### editor

Type: `string`

This can be pretty flexible. It matches against all the data it has.

For example, to get Sublime Text, you could write either of the following: `sublime`, `Sublime Text`, `subl`.

### allEditors()

Returns an array with metadata on all the editors.
