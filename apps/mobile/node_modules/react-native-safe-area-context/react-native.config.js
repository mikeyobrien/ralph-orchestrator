module.exports = {
  dependency: {
    platforms: {
      android: {
        libraryName: 'safeareacontext',
        componentDescriptors: [
          'RNCSafeAreaProviderComponentDescriptor',
          'RNCSafeAreaViewComponentDescriptor',
        ],
        cmakeListsPath: 'src/main/jni/CMakeLists.txt',
      },
      macos: null,
      windows: null,
    },
  },
};
