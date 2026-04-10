/// rules/perf/mod.rs — All 15 React performance rules.
pub mod large_component;
pub mod no_array_index_key;
pub mod no_component_in_component;
pub mod no_expensive_in_render;
pub mod no_inline_jsx_fn;
pub mod no_json_in_render;
pub mod no_math_random_in_render;
pub mod no_new_context_value;
pub mod no_new_in_jsx_prop;
pub mod no_object_entries_in_render;
pub mod no_regex_in_render;
pub mod no_unstable_hook_deps;
pub mod no_use_state_lazy_init_missing;
pub mod no_useless_memo;
pub mod unstable_props;

use crate::rules::Rule;

pub fn perf_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_inline_jsx_fn::NoInlineJsxFn),
        Box::new(unstable_props::UnstableProps),
        Box::new(large_component::LargeComponent),
        Box::new(no_new_context_value::NoNewContextValue),
        Box::new(no_array_index_key::NoArrayIndexKey),
        Box::new(no_expensive_in_render::NoExpensiveInRender),
        Box::new(no_component_in_component::NoComponentInComponent),
        Box::new(no_unstable_hook_deps::NoUnstableHookDeps),
        Box::new(no_new_in_jsx_prop::NoNewInJsxProp),
        Box::new(no_use_state_lazy_init_missing::NoUseStateLazyInitMissing),
        Box::new(no_json_in_render::NoJsonInRender),
        Box::new(no_object_entries_in_render::NoObjectEntriesInRender),
        Box::new(no_regex_in_render::NoRegexInRender),
        Box::new(no_math_random_in_render::NoMathRandomInRender),
        Box::new(no_useless_memo::NoUselessMemo),
    ]
}
