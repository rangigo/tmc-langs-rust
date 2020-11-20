initSidebarItems({"fn":[["block_on","Converts a stream into a blocking iterator."],["empty","Creates an empty stream."],["iter","Creates a stream from an iterator."],["once","Creates a stream that yields a single item."],["pending","Creates a stream that is always pending."],["poll_fn","Creates a stream from a function returning [`Poll`]."],["repeat","Creates an infinite stream that yields the same item repeatedly."],["repeat_with","Creates an infinite stream from a closure that generates items."],["try_unfold","Creates a stream from a seed value and a fallible async closure operating on it."],["unfold","Creates a stream from a seed value and an async closure operating on it."]],"struct":[["BlockOn","Iterator for the [`block_on()`] function."],["CollectFuture","Future for the [`StreamExt::collect()`] method."],["CountFuture","Future for the [`StreamExt::count()`] method."],["Empty","Stream for the [`empty()`] function."],["Filter","Stream for the [`StreamExt::filter()`] method."],["FilterMap","Stream for the [`StreamExt::filter_map()`] method."],["FoldFuture","Future for the [`StreamExt::fold()`] method."],["Iter","Stream for the [`iter()`] function."],["Map","Stream for the [`StreamExt::map()`] method."],["NextFuture","Future for the [`StreamExt::next()`] method."],["Once","Stream for the [`once()`] function."],["Pending","Stream for the [`pending()`] function."],["PollFn","Stream for the [`poll_fn()`] function."],["Repeat","Stream for the [`repeat()`] function."],["RepeatWith","Stream for the [`repeat_with()`] function."],["TryCollectFuture","Future for the [`StreamExt::try_collect()`] method."],["TryFoldFuture","Future for the [`StreamExt::try_fold()`] method."],["TryUnfold","Stream for the [`try_unfold()`] function."],["Unfold","Stream for the [`unfold()`] function."]],"trait":[["StreamExt","Extension trait for [`Stream`]."]],"type":[["Boxed","Type alias for `Pin<Box<dyn Stream<Item = T> + Send>>`."],["BoxedLocal","Type alias for `Pin<Box<dyn Stream<Item = T>>>`."]]});