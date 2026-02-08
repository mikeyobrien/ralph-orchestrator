"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.getAsset = getAsset;
exports.getAssetData = getAssetData;
exports.getAssetFiles = getAssetFiles;
exports.getAssetSize = getAssetSize;
exports.isAssetTypeAnImage = isAssetTypeAnImage;
var _pathUtils = require("./lib/pathUtils");
var AssetPaths = _interopRequireWildcard(
  require("./node-haste/lib/AssetPaths"),
);
var _crypto = _interopRequireDefault(require("crypto"));
var _fs = _interopRequireDefault(require("fs"));
var _imageSize = _interopRequireDefault(require("image-size"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function _getRequireWildcardCache(e) {
  if ("function" != typeof WeakMap) return null;
  var r = new WeakMap(),
    t = new WeakMap();
  return (_getRequireWildcardCache = function (e) {
    return e ? t : r;
  })(e);
}
function _interopRequireWildcard(e, r) {
  if (!r && e && e.__esModule) return e;
  if (null === e || ("object" != typeof e && "function" != typeof e))
    return { default: e };
  var t = _getRequireWildcardCache(r);
  if (t && t.has(e)) return t.get(e);
  var n = { __proto__: null },
    a = Object.defineProperty && Object.getOwnPropertyDescriptor;
  for (var u in e)
    if ("default" !== u && {}.hasOwnProperty.call(e, u)) {
      var i = a ? Object.getOwnPropertyDescriptor(e, u) : null;
      i && (i.get || i.set) ? Object.defineProperty(n, u, i) : (n[u] = e[u]);
    }
  return ((n.default = e), t && t.set(e, n), n);
}
function isAssetTypeAnImage(type) {
  return (
    [
      "png",
      "jpg",
      "jpeg",
      "bmp",
      "gif",
      "webp",
      "psd",
      "svg",
      "tiff",
      "ktx",
    ].indexOf(type) !== -1
  );
}
function getAssetSize(type, content, filePath) {
  if (!isAssetTypeAnImage(type)) {
    return null;
  }
  if (content.length === 0) {
    throw new Error(`Image asset \`${filePath}\` cannot be an empty file.`);
  }
  const { width, height } = (0, _imageSize.default)(content);
  return {
    width,
    height,
  };
}
function buildAssetMap(dir, files, platform) {
  const platforms = new Set(platform != null ? [platform] : []);
  const assets = files.map((file) => AssetPaths.tryParse(file, platforms));
  const map = new Map();
  assets.forEach(function (asset, i) {
    if (asset == null) {
      return;
    }
    const file = files[i];
    const assetKey = getAssetKey(asset.assetName, asset.platform);
    let record = map.get(assetKey);
    if (!record) {
      record = {
        scales: [],
        files: [],
      };
      map.set(assetKey, record);
    }
    let insertIndex;
    const length = record.scales.length;
    for (insertIndex = 0; insertIndex < length; insertIndex++) {
      if (asset.resolution < record.scales[insertIndex]) {
        break;
      }
    }
    record.scales.splice(insertIndex, 0, asset.resolution);
    record.files.splice(insertIndex, 0, _path.default.join(dir, file));
  });
  return map;
}
function getAssetKey(assetName, platform) {
  if (platform != null) {
    return `${assetName} : ${platform}`;
  } else {
    return assetName;
  }
}
async function getAbsoluteAssetRecord(assetPath, platform = null) {
  const filename = _path.default.basename(assetPath);
  const dir = _path.default.dirname(assetPath);
  const files = await _fs.default.promises.readdir(dir);
  const assetData = AssetPaths.parse(
    filename,
    new Set(platform != null ? [platform] : []),
  );
  const map = buildAssetMap(dir, files, platform);
  let record;
  if (platform != null) {
    record =
      map.get(getAssetKey(assetData.assetName, platform)) ||
      map.get(assetData.assetName);
  } else {
    record = map.get(assetData.assetName);
  }
  if (!record) {
    throw new Error(
      `Asset not found: ${assetPath} for platform: ${platform ?? "(unspecified)"}`,
    );
  }
  return record;
}
async function getAbsoluteAssetInfo(assetPath, platform = null) {
  const nameData = AssetPaths.parse(
    assetPath,
    new Set(platform != null ? [platform] : []),
  );
  const { name, type } = nameData;
  const { scales, files } = await getAbsoluteAssetRecord(assetPath, platform);
  const hasher = _crypto.default.createHash("md5");
  const fileData = await Promise.all(
    files.map((file) => _fs.default.promises.readFile(file)),
  );
  for (const data of fileData) {
    hasher.update(data);
  }
  return {
    files,
    hash: hasher.digest("hex"),
    name,
    scales,
    type,
  };
}
async function getAssetData(
  assetPath,
  localPath,
  assetDataPlugins,
  platform = null,
  publicPath,
) {
  let assetUrlPath = localPath.startsWith("..")
    ? publicPath.replace(/\/$/, "") + "/" + _path.default.dirname(localPath)
    : _path.default.join(publicPath, _path.default.dirname(localPath));
  assetUrlPath = (0, _pathUtils.normalizePathSeparatorsToPosix)(assetUrlPath);
  const isImage = isAssetTypeAnImage(_path.default.extname(assetPath).slice(1));
  const assetInfo = await getAbsoluteAssetInfo(assetPath, platform);
  const isImageInput = assetInfo.files[0].includes(".zip/")
    ? _fs.default.readFileSync(assetInfo.files[0])
    : assetInfo.files[0];
  const dimensions = isImage ? (0, _imageSize.default)(isImageInput) : null;
  const scale = assetInfo.scales[0];
  const assetData = {
    __packager_asset: true,
    fileSystemLocation: _path.default.dirname(assetPath),
    httpServerLocation: assetUrlPath,
    width: dimensions ? dimensions.width / scale : undefined,
    height: dimensions ? dimensions.height / scale : undefined,
    scales: assetInfo.scales,
    files: assetInfo.files,
    hash: assetInfo.hash,
    name: assetInfo.name,
    type: assetInfo.type,
  };
  return await applyAssetDataPlugins(assetDataPlugins, assetData);
}
async function applyAssetDataPlugins(assetDataPlugins, assetData) {
  if (!assetDataPlugins.length) {
    return assetData;
  }
  const [currentAssetPlugin, ...remainingAssetPlugins] = assetDataPlugins;
  const assetPluginFunction = require(currentAssetPlugin);
  const resultAssetData = await assetPluginFunction(assetData);
  return await applyAssetDataPlugins(remainingAssetPlugins, resultAssetData);
}
async function getAssetFiles(assetPath, platform = null) {
  const assetData = await getAbsoluteAssetRecord(assetPath, platform);
  return assetData.files;
}
async function getAsset(
  relativePath,
  projectRoot,
  watchFolders,
  platform = null,
  assetExts,
) {
  const assetData = AssetPaths.parse(
    relativePath,
    new Set(platform != null ? [platform] : []),
  );
  const absolutePath = _path.default.resolve(projectRoot, relativePath);
  if (!assetExts.includes(assetData.type)) {
    throw new Error(
      `'${relativePath}' cannot be loaded as its extension is not registered in assetExts`,
    );
  }
  if (!pathBelongsToRoots(absolutePath, [projectRoot, ...watchFolders])) {
    throw new Error(
      `'${relativePath}' could not be found, because it cannot be found in the project root or any watch folder`,
    );
  }
  const record = await getAbsoluteAssetRecord(absolutePath, platform);
  for (let i = 0; i < record.scales.length; i++) {
    if (record.scales[i] >= assetData.resolution) {
      return _fs.default.promises.readFile(record.files[i]);
    }
  }
  return _fs.default.promises.readFile(record.files[record.files.length - 1]);
}
function pathBelongsToRoots(pathToCheck, roots) {
  for (const rootFolder of roots) {
    if (pathToCheck.startsWith(_path.default.resolve(rootFolder))) {
      return true;
    }
  }
  return false;
}
