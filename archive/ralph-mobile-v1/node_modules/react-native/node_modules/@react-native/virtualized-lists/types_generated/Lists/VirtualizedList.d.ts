/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<8997e25a6b501ab434c0037b6347705b>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/VirtualizedList.js
 */

import type { Item, ListRenderItem, ListRenderItemInfo, Separators, VirtualizedListProps } from "./VirtualizedListProps";
import type { ScrollEvent, ScrollResponderType } from "react-native";
import { CellRenderMask } from "./CellRenderMask";
import StateSafePureComponent from "./StateSafePureComponent";
import { VirtualizedListContext } from "./VirtualizedListContext.js";
import * as React from "react";
import { ScrollView } from "react-native";
export type { ListRenderItemInfo, ListRenderItem, Separators };
type State = {
  renderMask: CellRenderMask;
  cellsAroundViewport: {
    first: number;
    last: number;
  };
  firstVisibleItemKey: string | undefined;
  pendingScrollUpdateCount: number;
};
/**
 * Base implementation for the more convenient [`<FlatList>`](https://reactnative.dev/docs/flatlist)
 * and [`<SectionList>`](https://reactnative.dev/docs/sectionlist) components, which are also better
 * documented. In general, this should only really be used if you need more flexibility than
 * `FlatList` provides, e.g. for use with immutable data instead of plain arrays.
 *
 * Virtualization massively improves memory consumption and performance of large lists by
 * maintaining a finite render window of active items and replacing all items outside of the render
 * window with appropriately sized blank space. The window adapts to scrolling behavior, and items
 * are rendered incrementally with low-pri (after any running interactions) if they are far from the
 * visible area, or with hi-pri otherwise to minimize the potential of seeing blank space.
 *
 * Some caveats:
 *
 * - Internal state is not preserved when content scrolls out of the render window. Make sure all
 *   your data is captured in the item data or external stores like Flux, Redux, or Relay.
 * - This is a `PureComponent` which means that it will not re-render if `props` remain shallow-
 *   equal. Make sure that everything your `renderItem` function depends on is passed as a prop
 *   (e.g. `extraData`) that is not `===` after updates, otherwise your UI may not update on
 *   changes. This includes the `data` prop and parent component state.
 * - In order to constrain memory and enable smooth scrolling, content is rendered asynchronously
 *   offscreen. This means it's possible to scroll faster than the fill rate ands momentarily see
 *   blank content. This is a tradeoff that can be adjusted to suit the needs of each application,
 *   and we are working on improving it behind the scenes.
 * - By default, the list looks for a `key` or `id` prop on each item and uses that for the React key.
 *   Alternatively, you can provide a custom `keyExtractor` prop.
 * - As an effort to remove defaultProps, use helper functions when referencing certain props
 *
 */
declare class VirtualizedList extends StateSafePureComponent<VirtualizedListProps, State> {
  static contextType: typeof VirtualizedListContext;
  scrollToEnd(params?: null | undefined | {
    animated?: boolean | undefined;
  }): void;
  scrollToIndex(params: {
    animated?: boolean | undefined;
    index: number;
    viewOffset?: number;
    viewPosition?: number;
  }): any;
  scrollToItem(params: {
    animated?: boolean | undefined;
    item: Item;
    viewOffset?: number;
    viewPosition?: number;
  }): void;
  /**
   * Scroll to a specific content pixel offset in the list.
   *
   * Param `offset` expects the offset to scroll to.
   * In case of `horizontal` is true, the offset is the x-value,
   * in any other case the offset is the y-value.
   *
   * Param `animated` (`true` by default) defines whether the list
   * should do an animation while scrolling.
   */
  scrollToOffset(params: {
    animated?: boolean | undefined;
    offset: number;
  }): void;
  recordInteraction(): void;
  flashScrollIndicators(): void;
  /**
   * Provides a handle to the underlying scroll responder.
   * Note that `this._scrollRef` might not be a `ScrollView`, so we
   * need to check that it responds to `getScrollResponder` before calling it.
   */
  getScrollResponder(): null | undefined | ScrollResponderType;
  getScrollableNode(): null | undefined | number;
  getScrollRef(): null | undefined | React.ComponentRef<typeof ScrollView>;
  setNativeProps(props: Object): void;
  hasMore(): boolean;
  state: State;
  constructor(props: VirtualizedListProps);
  componentDidMount(): void;
  componentWillUnmount(): void;
  static getDerivedStateFromProps(newProps: VirtualizedListProps, prevState: State): State;
  render(): React.ReactNode;
  componentDidUpdate(prevProps: VirtualizedListProps): void;
  measureLayoutRelativeToContainingList(): void;
  unstable_onScroll(e: Object): void;
  unstable_onScrollBeginDrag(e: ScrollEvent): void;
  unstable_onScrollEndDrag(e: ScrollEvent): void;
  unstable_onMomentumScrollBegin(e: ScrollEvent): void;
  unstable_onMomentumScrollEnd(e: ScrollEvent): void;
}
export default VirtualizedList;
