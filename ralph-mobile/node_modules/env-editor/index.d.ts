export interface Editor {
	id: string;
	name: string;
	binary: string;
	isTerminalEditor: boolean;
	paths: string[];
	keywords: string[];
}

/**
Get info about the default editor.

The user is expected to have the `$EDITOR` environment variable set, and if not, a user-friendly error is thrown.

@returns Metadata on the default editor.

@example
```
import {defaultEditor} from 'env-editor';

defaultEditor();
// {
// 	id: 'atom',
// 	name: 'Atom',
// 	binary: 'atom',
// 	isTerminalEditor: false,
// 	paths: [
// 		'/Applications/Atom.app/Contents/Resources/app/atom.sh'
// 	],
// 	keywords: []
// }
```
*/
export function defaultEditor(): Editor;

/**
Get info about a specific editor.

@param editor - This can be pretty flexible. It matches against all the data it has. For example, to get Sublime Text, you could write either of the following: `sublime`, `Sublime Text`, `subl`.
@returns Metadata on the specified editor.

@example
```
import {getEditor} from 'env-editor';

getEditor('sublime');
// {
// 	id: 'sublime',
// 	name: 'Sublime Text',
// 	binary: 'subl',
// 	isTerminalEditor: false,
// 	paths: [
// 		'/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl',
// 		'/Applications/Sublime Text 2.app/Contents/SharedSupport/bin/subl'
// 	],
// 	keywords: []
// }
```
*/
export function getEditor(editor: string): Editor;

/**
@returns Metadata on all the editors.

@example
```
import {allEditors} from 'env-editor';

allEditors();
// [
// 	{
// 		id: 'atom',
// 		…
// 	},
// 	{
// 		id: 'sublime,
// 		…
// 	},
// 	…
// ]
```
*/
export function allEditors(): Editor[];
