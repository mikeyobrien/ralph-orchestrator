"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = importExportPlugin;
var _template = _interopRequireDefault(require("@babel/template"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const importTemplate = _template.default.statement(`
  var LOCAL = IMPORT(FILE);
`);
const importNamedTemplate = _template.default.statement(`
  var LOCAL = require(FILE).REMOTE;
`);
const importSideEffectTemplate = _template.default.statement(`
  require(FILE);
`);
const exportAllTemplate = _template.default.statements(`
  var REQUIRED = require(FILE);

  for (var KEY in REQUIRED) {
    exports[KEY] = REQUIRED[KEY];
  }
`);
const exportTemplate = _template.default.statement(`
  exports.REMOTE = LOCAL;
`);
const esModuleExportTemplate = _template.default.statement(`
  Object.defineProperty(exports, '__esModule', {value: true});
`);
const resolveTemplate = _template.default.expression(`
  require.resolve(NODE)
`);
function resolvePath(node, resolve) {
  if (!resolve) {
    return node;
  }
  return resolveTemplate({
    NODE: node,
  });
}
function withLocation(node, loc) {
  if (Array.isArray(node)) {
    return node.map((n) => withLocation(n, loc));
  }
  if (!node.loc) {
    return {
      ...node,
      loc,
    };
  }
  return node;
}
function importExportPlugin({ types: t }) {
  const { isDeclaration, isVariableDeclaration } = t;
  return {
    visitor: {
      ExportAllDeclaration(path, state) {
        state.exportAll.push({
          file: path.node.source.value,
          loc: path.node.loc,
        });
        path.remove();
      },
      ExportDefaultDeclaration(path, state) {
        const declaration = path.node.declaration;
        const id =
          declaration.id || path.scope.generateUidIdentifier("default");
        declaration.id = id;
        const loc = path.node.loc;
        state.exportDefault.push({
          local: id.name,
          loc,
        });
        if (isDeclaration(declaration)) {
          path.insertBefore(withLocation(declaration, loc));
        } else {
          path.insertBefore(
            withLocation(
              t.variableDeclaration("var", [
                t.variableDeclarator(id, declaration),
              ]),
              loc,
            ),
          );
        }
        path.remove();
      },
      ExportNamedDeclaration(path, state) {
        if (path.node.exportKind && path.node.exportKind !== "value") {
          return;
        }
        const declaration = path.node.declaration;
        const loc = path.node.loc;
        if (declaration) {
          if (isVariableDeclaration(declaration)) {
            declaration.declarations.forEach((d) => {
              switch (d.id.type) {
                case "ObjectPattern":
                  {
                    const properties = d.id.properties;
                    properties.forEach((p) => {
                      const name = p.key.name;
                      state.exportNamed.push({
                        local: name,
                        remote: name,
                        loc,
                      });
                    });
                  }
                  break;
                case "ArrayPattern":
                  {
                    const elements = d.id.elements;
                    elements.forEach((e) => {
                      const name = e.name;
                      state.exportNamed.push({
                        local: name,
                        remote: name,
                        loc,
                      });
                    });
                  }
                  break;
                default:
                  {
                    const name = d.id.name;
                    state.exportNamed.push({
                      local: name,
                      remote: name,
                      loc,
                    });
                  }
                  break;
              }
            });
          } else {
            const id = declaration.id || path.scope.generateUidIdentifier();
            const name = id.name;
            declaration.id = id;
            state.exportNamed.push({
              local: name,
              remote: name,
              loc,
            });
          }
          path.insertBefore(declaration);
        }
        const specifiers = path.node.specifiers;
        if (specifiers) {
          specifiers.forEach((s) => {
            const local = s.local;
            const remote = s.exported;
            if (remote.type === "StringLiteral") {
              throw path.buildCodeFrameError(
                "Module string names are not supported",
              );
            }
            if (path.node.source) {
              const temp = path.scope.generateUidIdentifier(local.name);
              if (local.name === "default") {
                path.insertBefore(
                  withLocation(
                    importTemplate({
                      IMPORT: t.cloneNode(state.importDefault),
                      FILE: resolvePath(
                        t.cloneNode((0, _nullthrows.default)(path.node.source)),
                        state.opts.resolve,
                      ),
                      LOCAL: temp,
                    }),
                    loc,
                  ),
                );
                state.exportNamed.push({
                  local: temp.name,
                  remote: remote.name,
                  loc,
                });
              } else if (remote.name === "default") {
                path.insertBefore(
                  withLocation(
                    importNamedTemplate({
                      FILE: resolvePath(
                        t.cloneNode((0, _nullthrows.default)(path.node.source)),
                        state.opts.resolve,
                      ),
                      LOCAL: temp,
                      REMOTE: local,
                    }),
                    loc,
                  ),
                );
                state.exportDefault.push({
                  local: temp.name,
                  loc,
                });
              } else {
                path.insertBefore(
                  withLocation(
                    importNamedTemplate({
                      FILE: resolvePath(
                        t.cloneNode((0, _nullthrows.default)(path.node.source)),
                        state.opts.resolve,
                      ),
                      LOCAL: temp,
                      REMOTE: local,
                    }),
                    loc,
                  ),
                );
                state.exportNamed.push({
                  local: temp.name,
                  remote: remote.name,
                  loc,
                });
              }
            } else {
              if (remote.name === "default") {
                state.exportDefault.push({
                  local: local.name,
                  loc,
                });
              } else {
                state.exportNamed.push({
                  local: local.name,
                  remote: remote.name,
                  loc,
                });
              }
            }
          });
        }
        path.remove();
      },
      ImportDeclaration(path, state) {
        if (path.node.importKind && path.node.importKind !== "value") {
          return;
        }
        const file = path.node.source;
        const specifiers = path.node.specifiers;
        const loc = path.node.loc;
        if (!specifiers.length) {
          state.imports.push({
            node: withLocation(
              importSideEffectTemplate({
                FILE: resolvePath(t.cloneNode(file), state.opts.resolve),
              }),
              loc,
            ),
          });
        } else {
          let sharedModuleImport;
          let sharedModuleVariableDeclaration = null;
          if (
            specifiers.filter(
              (s) =>
                s.type === "ImportSpecifier" &&
                (s.imported.type === "StringLiteral" ||
                  s.imported.name !== "default"),
            ).length > 1
          ) {
            sharedModuleImport =
              path.scope.generateUidIdentifierBasedOnNode(file);
            sharedModuleVariableDeclaration = withLocation(
              t.variableDeclaration("var", [
                t.variableDeclarator(
                  t.cloneNode(sharedModuleImport),
                  t.callExpression(t.identifier("require"), [
                    resolvePath(t.cloneNode(file), state.opts.resolve),
                  ]),
                ),
              ]),
              loc,
            );
            state.imports.push({
              node: sharedModuleVariableDeclaration,
            });
          }
          specifiers.forEach((s) => {
            const imported = s.imported;
            const local = s.local;
            switch (s.type) {
              case "ImportNamespaceSpecifier":
                state.imports.push({
                  node: withLocation(
                    importTemplate({
                      IMPORT: t.cloneNode(state.importAll),
                      FILE: resolvePath(t.cloneNode(file), state.opts.resolve),
                      LOCAL: t.cloneNode(local),
                    }),
                    loc,
                  ),
                });
                break;
              case "ImportDefaultSpecifier":
                state.imports.push({
                  node: withLocation(
                    importTemplate({
                      IMPORT: t.cloneNode(state.importDefault),
                      FILE: resolvePath(t.cloneNode(file), state.opts.resolve),
                      LOCAL: t.cloneNode(local),
                    }),
                    loc,
                  ),
                });
                break;
              case "ImportSpecifier":
                if (imported.name === "default") {
                  state.imports.push({
                    node: withLocation(
                      importTemplate({
                        IMPORT: t.cloneNode(state.importDefault),
                        FILE: resolvePath(
                          t.cloneNode(file),
                          state.opts.resolve,
                        ),
                        LOCAL: t.cloneNode(local),
                      }),
                      loc,
                    ),
                  });
                } else if (sharedModuleVariableDeclaration != null) {
                  sharedModuleVariableDeclaration.declarations.push(
                    withLocation(
                      t.variableDeclarator(
                        t.cloneNode(local),
                        t.memberExpression(
                          t.cloneNode(sharedModuleImport),
                          t.cloneNode(imported),
                        ),
                      ),
                      loc,
                    ),
                  );
                } else {
                  state.imports.push({
                    node: withLocation(
                      importNamedTemplate({
                        FILE: resolvePath(
                          t.cloneNode(file),
                          state.opts.resolve,
                        ),
                        LOCAL: t.cloneNode(local),
                        REMOTE: t.cloneNode(imported),
                      }),
                      loc,
                    ),
                  });
                }
                break;
              default:
                throw new TypeError("Unknown import type: " + s.type);
            }
          });
        }
        path.remove();
      },
      Program: {
        enter(path, state) {
          state.exportAll = [];
          state.exportDefault = [];
          state.exportNamed = [];
          state.imports = [];
          state.importAll = t.identifier(state.opts.importAll);
          state.importDefault = t.identifier(state.opts.importDefault);
          ["module", "global", "exports", "require"].forEach((name) =>
            path.scope.rename(name),
          );
        },
        exit(path, state) {
          const body = path.node.body;
          state.imports.reverse().forEach((e) => {
            body.unshift(e.node);
          });
          state.exportDefault.forEach((e) => {
            body.push(
              withLocation(
                exportTemplate({
                  LOCAL: t.identifier(e.local),
                  REMOTE: t.identifier("default"),
                }),
                e.loc,
              ),
            );
          });
          state.exportAll.forEach((e) => {
            body.push(
              ...withLocation(
                exportAllTemplate({
                  FILE: resolvePath(
                    t.stringLiteral(e.file),
                    state.opts.resolve,
                  ),
                  REQUIRED: path.scope.generateUidIdentifier(e.file),
                  KEY: path.scope.generateUidIdentifier("key"),
                }),
                e.loc,
              ),
            );
          });
          state.exportNamed.forEach((e) => {
            body.push(
              withLocation(
                exportTemplate({
                  LOCAL: t.identifier(e.local),
                  REMOTE: t.identifier(e.remote),
                }),
                e.loc,
              ),
            );
          });
          if (
            state.exportDefault.length ||
            state.exportAll.length ||
            state.exportNamed.length
          ) {
            body.unshift(esModuleExportTemplate());
            if (state.opts.out) {
              state.opts.out.isESModule = true;
            }
          } else if (state.opts.out) {
            state.opts.out.isESModule = false;
          }
        },
      },
    },
  };
}
