//! Conservative author-facing suggestions from `web-lua-suggestions.ts`.

pub fn suggest_web_lua_name<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let needle = normalize(input);
    if needle.is_empty() {
        return None;
    }
    let limit = (needle.len() / 3).clamp(1, 3);
    let mut best: Option<(&str, usize)> = None;
    for candidate in candidates.into_iter().take(4_096) {
        if candidate == input {
            continue;
        }
        let target = normalize(candidate);
        if target.is_empty() || target.len().abs_diff(needle.len()) > limit {
            continue;
        }
        let distance = edit_distance(&needle, &target, limit);
        if distance > limit {
            continue;
        }
        let preferred = best.is_none_or(|(previous, previous_distance)| {
            distance < previous_distance
                || distance == previous_distance
                    && (
                        candidate.encode_utf16().count(),
                        candidate.encode_utf16().collect::<Vec<_>>(),
                    ) < (
                        previous.encode_utf16().count(),
                        previous.encode_utf16().collect::<Vec<_>>(),
                    )
        });
        if preferred {
            best = Some((candidate, distance));
        }
    }
    best.map(|(candidate, _)| candidate)
}

pub fn did_you_mean<'a>(input: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    suggest_web_lua_name(input, candidates)
        .map_or_else(String::new, |candidate| format!("; did you mean {candidate}?"))
}

pub fn list_choices<'a>(candidates: impl IntoIterator<Item = &'a str>) -> String {
    let values: Vec<_> = candidates.into_iter().collect();
    if values.len() <= 12 {
        return values.join(", ");
    }
    format!("{}, and {} more", values[..12].join(", "), values.len() - 12)
}

fn normalize(value: &str) -> Vec<u16> {
    value.to_lowercase().replace(['-', ' '], "_").encode_utf16().collect()
}

fn edit_distance(left: &[u16], right: &[u16], limit: usize) -> usize {
    let mut two_back: Option<Vec<usize>> = None;
    let mut previous: Vec<_> = (0..=right.len()).collect();
    for row in 1..=left.len() {
        let mut current = vec![row];
        let mut row_minimum = row;
        for column in 1..=right.len() {
            let substitution =
                previous[column - 1] + usize::from(left[row - 1] != right[column - 1]);
            let mut distance =
                (previous[column] + 1).min(current[column - 1] + 1).min(substitution);
            if let Some(two_back) = &two_back
                && row > 1
                && column > 1
                && left[row - 1] == right[column - 2]
                && left[row - 2] == right[column - 1]
            {
                distance = distance.min(two_back[column - 2] + 1);
            }
            current.push(distance);
            row_minimum = row_minimum.min(distance);
        }
        if row_minimum > limit {
            return limit + 1;
        }
        two_back = Some(previous);
        previous = current;
    }
    previous[right.len()]
}
