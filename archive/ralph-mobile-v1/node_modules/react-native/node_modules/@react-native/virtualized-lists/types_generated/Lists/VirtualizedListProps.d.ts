/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<fcf224b18f3315e5b84e224781b4eeb1>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/VirtualizedListProps.js
 */

import type { ViewabilityConfig, ViewabilityConfigCallbackPair, ViewToken } from "./ViewabilityHelper";
import type { FocusEvent, LayoutChangeEvent, ScrollViewProps, StyleProp, ViewStyle } from "react-native";
import * as React from "react";
export type Item = any;
export type Separators = {
  highlight: () => void;
  unhighlight: () => void;
  updateProps: (select: "leading" | "trailing", newProps: Object) => void;
};
export type ListRenderItemInfo<ItemT> = {
  item: ItemT;
  index: number;
  separators: Separators;
};
export type CellRendererProps<ItemT> = Readonly<{
  cellKey: string;
  children: React.ReactNode;
  index: number;
  item: ItemT;
  onFocusCapture?: (event: FocusEvent) => void;
  onLayout?: (event: LayoutChangeEvent) => void;
  style: StyleProp<ViewStyle>;
}>;
export type ListRenderItem<ItemT> = (info: ListRenderItemInfo<ItemT>) => React.ReactNode;
type RequiredVirtualizedListProps = {
  /**
   * The default accessor functions assume this is an Array<{key: string} | {id: string}> but you can override
   * getItem, getItemCount, and keyExtractor to handle any type of index-based data.
   */
  data?: any;
  /**
   * A generic accessor for extracting an item from any sort of data blob.
   */
  getItem: (data: any, index: number) => Item | undefined;
  /**
   * Determines how many items are in the data blob.
   */
  getItemCount: (data: any) => number;
};
type OptionalVirtualizedListProps = {
  renderItem?: ListRenderItem<Item> | undefined;
  /**
   * `debug` will turn on extra logging and visual overlays to aid with debugging both usage and
   * implementation, but with a significant perf hit.
   */
  debug?: boolean | undefined;
  /**
   * DEPRECATED: Virtualization provides significant performance and memory optimizations, but fully
   * unmounts react instances that are outside of the render window. You should only need to disable
   * this for debugging purposes. Defaults to false.
   */
  disableVirtualization?: boolean | undefined;
  /**
   * A marker property for telling the list to re-render (since it implements `PureComponent`). If
   * any of your `renderItem`, Header, Footer, etc. functions depend on anything outside of the
   * `data` prop, stick it here and treat it immutably.
   */
  extraData?: any;
  getItemLayout?: (data: any, index: number) => {
    length: number;
    offset: number;
    index: number;
  };
  horizontal?: boolean | undefined;
  /**
   * How many items to render in the initial batch. This should be enough to fill the screen but not
   * much more. Note these items will never be unmounted as part of the windowed rendering in order
   * to improve perceived performance of scroll-to-top actions.
   */
  initialNumToRender?: number | undefined;
  /**
   * Instead of starting at the top with the first item, start at `initialScrollIndex`. This
   * disables the "scroll to top" optimization that keeps the first `initialNumToRender` items
   * always rendered and immediately renders the items starting at this initial index. Requires
   * `getItemLayout` to be implemented.
   */
  initialScrollIndex?: number | undefined;
  /**
   * Reverses the direction of scroll. Uses scale transforms of -1.
   */
  inverted?: boolean | undefined;
  keyExtractor?: ((item: Item, index: number) => string) | undefined;
  /**
   * CellRendererComponent allows customizing how cells rendered by
   * `renderItem`/`ListItemComponent` are wrapped when placed into the
   * underlying ScrollView. This component must accept event handlers which
   * notify VirtualizedList of changes within the cell.
   */
  CellRendererComponent?: React.ComponentType<CellRendererProps<Item>> | undefined;
  /**
   * Rendered in between each item, but not at the top or bottom. By default, `highlighted` and
   * `leadingItem` props are provided. `renderItem` provides `separators.highlight`/`unhighlight`
   * which will update the `highlighted` prop, but you can also add custom props with
   * `separators.updateProps`.
   */
  ItemSeparatorComponent?: React.ComponentType<any> | undefined;
  /**
   * Takes an item from `data` and renders it into the list. Example usage:
   *
   *     <FlatList
   *       ItemSeparatorComponent={Platform.OS !== 'android' && ({highlighted}) => (
   *         <View style={[style.separator, highlighted && {marginLeft: 0}]} />
   *       )}
   *       data={[{title: 'Title Text', key: 'item1'}]}
   *       ListItemComponent={({item, separators}) => (
   *         <TouchableHighlight
   *           onPress={() => this._onPress(item)}
   *           onShowUnderlay={separators.highlight}
   *           onHideUnderlay={separators.unhighlight}>
   *           <View style={{backgroundColor: 'white'}}>
   *             <Text>{item.title}</Text>
   *           </View>
   *         </TouchableHighlight>
   *       )}
   *     />
   *
   * Provides additional metadata like `index` if you need it, as well as a more generic
   * `separators.updateProps` function which let's you set whatever props you want to change the
   * rendering of either the leading separator or trailing separator in case the more common
   * `highlight` and `unhighlight` (which set the `highlighted: boolean` prop) are insufficient for
   * your use-case.
   */
  ListItemComponent?: (React.ComponentType<any> | React.JSX.Element) | undefined;
  /**
   * Rendered when the list is empty. Can be a React Component Class, a render function, or
   * a rendered element.
   */
  ListEmptyComponent?: (React.ComponentType<any> | React.JSX.Element) | undefined;
  /**
   * Rendered at the bottom of all the items. Can be a React Component Class, a render function, or
   * a rendered element.
   */
  ListFooterComponent?: (React.ComponentType<any> | React.JSX.Element) | undefined;
  /**
   * Styling for internal View for ListFooterComponent
   */
  ListFooterComponentStyle?: StyleProp<ViewStyle>;
  /**
   * Rendered at the top of all the items. Can be a React Component Class, a render function, or
   * a rendered element.
   */
  ListHeaderComponent?: (React.ComponentType<any> | React.JSX.Element) | undefined;
  /**
   * Styling for internal View for ListHeaderComponent
   */
  ListHeaderComponentStyle?: StyleProp<ViewStyle>;
  /**
   * The maximum number of items to render in each incremental render batch. The more rendered at
   * once, the better the fill rate, but responsiveness may suffer because rendering content may
   * interfere with responding to button taps or other interactions.
   */
  maxToRenderPerBatch?: number | undefined;
  /**
   * Called once when the scroll position gets within within `onEndReachedThreshold`
   * from the logical end of the list.
   */
  onEndReached?: ((info: {
    distanceFromEnd: number;
  }) => void) | undefined;
  /**
   * How far from the end (in units of visible length of the list) the trailing edge of the
   * list must be from the end of the content to trigger the `onEndReached` callback.
   * Thus, a value of 0.5 will trigger `onEndReached` when the end of the content is
   * within half the visible length of the list.
   */
  onEndReachedThreshold?: number | undefined;
  /**
   * If provided, a standard RefreshControl will be added for "Pull to Refresh" functionality. Make
   * sure to also set the `refreshing` prop correctly.
   */
  onRefresh?: (() => void) | undefined;
  /**
   * Used to handle failures when scrolling to an index that has not been measured yet. Recommended
   * action is to either compute your own offset and `scrollTo` it, or scroll as far as possible and
   * then try again after more items have been rendered.
   */
  onScrollToIndexFailed?: ((info: {
    index: number;
    highestMeasuredFrameIndex: number;
    averageItemLength: number;
  }) => void) | undefined;
  /**
   * Called once when the scroll position gets within within `onStartReachedThreshold`
   * from the logical start of the list.
   */
  onStartReached?: ((info: {
    distanceFromStart: number;
  }) => void) | undefined;
  /**
   * How far from the start (in units of visible length of the list) the leading edge of the
   * list must be from the start of the content to trigger the `onStartReached` callback.
   * Thus, a value of 0.5 will trigger `onStartReached` when the start of the content is
   * within half the visible length of the list.
   */
  onStartReachedThreshold?: number | undefined;
  /**
   * Called when the viewability of rows changes, as defined by the
   * `viewabilityConfig` prop.
   */
  onViewableItemsChanged?: ((info: {
    viewableItems: Array<ViewToken>;
    changed: Array<ViewToken>;
  }) => void) | undefined;
  persistentScrollbar?: boolean | undefined;
  /**
   * Set this when offset is needed for the loading indicator to show correctly.
   */
  progressViewOffset?: number;
  /**
   * A custom refresh control element. When set, it overrides the default
   * <RefreshControl> component built internally. The onRefresh and refreshing
   * props are also ignored. Only works for vertical VirtualizedList.
   */
  refreshControl?: React.JSX.Element | undefined;
  /**
   * Set this true while waiting for new data from a refresh.
   */
  refreshing?: boolean | undefined;
  /**
   * Note: may have bugs (missing content) in some circumstances - use at your own risk.
   *
   * This may improve scroll performance for large lists.
   */
  removeClippedSubviews?: boolean;
  /**
   * Render a custom scroll component, e.g. with a differently styled `RefreshControl`.
   */
  renderScrollComponent?: (props: ScrollViewProps) => React.JSX.Element;
  /**
   * Amount of time between low-pri item render batches, e.g. for rendering items quite a ways off
   * screen. Similar fill rate/responsiveness tradeoff as `maxToRenderPerBatch`.
   */
  updateCellsBatchingPeriod?: number | undefined;
  /**
   * See `ViewabilityHelper` for flow type and further documentation.
   */
  viewabilityConfig?: ViewabilityConfig;
  /**
   * List of ViewabilityConfig/onViewableItemsChanged pairs. A specific onViewableItemsChanged
   * will be called when its corresponding ViewabilityConfig's conditions are met.
   */
  viewabilityConfigCallbackPairs?: Array<ViewabilityConfigCallbackPair>;
  /**
   * Determines the maximum number of items rendered outside of the visible area, in units of
   * visible lengths. So if your list fills the screen, then `windowSize={21}` (the default) will
   * render the visible screen area plus up to 10 screens above and 10 below the viewport. Reducing
   * this number will reduce memory consumption and may improve performance, but will increase the
   * chance that fast scrolling may reveal momentary blank areas of unrendered content.
   */
  windowSize?: number | undefined;
};
export type VirtualizedListProps = Omit<ScrollViewProps, keyof RequiredVirtualizedListProps | keyof OptionalVirtualizedListProps | keyof {}> & Omit<RequiredVirtualizedListProps, keyof OptionalVirtualizedListProps | keyof {}> & Omit<OptionalVirtualizedListProps, keyof {}> & {};
/**
 * Default Props Helper Functions
 * Use the following helper functions for default values
 */

export declare function horizontalOrDefault(horizontal: null | undefined | boolean): boolean;
export declare function initialNumToRenderOrDefault(initialNumToRender: null | undefined | number): number;
export declare function maxToRenderPerBatchOrDefault(maxToRenderPerBatch: null | undefined | number): number;
export declare function onStartReachedThresholdOrDefault(onStartReachedThreshold: null | undefined | number): number;
export declare function onEndReachedThresholdOrDefault(onEndReachedThreshold: null | undefined | number): number;
export declare function windowSizeOrDefault(windowSize: null | undefined | number): number;
