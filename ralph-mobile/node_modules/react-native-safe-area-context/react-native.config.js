let supportsCodegenConfig = true;
try {
  const rnCliAndroidVersion = require.main.require(
    '@react-native-community/cli-platform-android/package.json',
  ).version;
  const [major] = rnCliAndroidVersion.split('.');
  supportsCodegenConfig = major >= 9;
} catch (e) {
  // ignore
}

module.exports = {
  dependency: {
    platforms: {
      android: supportsCodegenConfig
        ? {
            libraryName: 'safeareacontext',
            componentDescriptors: [
              'RNCSafeAreaProviderComponentDescriptor',
              'RNCSafeAreaViewComponentDescriptor',
            ],
            cmakeListsPath: 'src/main/jni/CMakeLists.txt',
          }
        : {},
      macos: null,
      windows: null,
    },
  },
};
