diesel::table! {
    artifacts (id) {
        id -> Nullable<Integer>,
        user -> Nullable<Integer>,
        challenge -> Integer,
        flag -> Text,
        info -> Text,
        path -> Text,
    }
}

diesel::table! {
    challenges (id) {
        id -> Nullable<Integer>,
        name -> Text,
        description -> Text,
        path -> Text,
        initial -> Double,
        points -> Double,
        problemset -> Nullable<Integer>,
        attachments -> Text,
        flag -> Text,
        dynamic -> Bool,
        public -> Bool,
    }
}

diesel::table! {
    problemsets (id) {
        id -> Nullable<Integer>,
        name -> Text,
    }
}

diesel::table! {
    solved (id) {
        id -> Nullable<Integer>,
        submission -> Integer,
        factor -> Double,
    }
}

diesel::table! {
    submissions (id) {
        id -> Nullable<Integer>,
        user -> Integer,
        challenge -> Integer,
        flag -> Text,
        time -> Timestamp,
    }
}

diesel::table! {
    use crate::db::models::UserRoleMapping;
    use diesel::sql_types::{Nullable, Integer, Text, Bool};

    users (id) {
        id -> Nullable<Integer>,
        username -> Text,
        password -> Text,
        contact -> Text,
        email -> Text,
        enabled -> Bool,
        role -> UserRoleMapping,
    }
}

diesel::joinable!(artifacts -> challenges (challenge));
diesel::joinable!(artifacts -> users (user));
diesel::joinable!(challenges -> problemsets (problemset));
diesel::joinable!(solved -> submissions (submission));
diesel::joinable!(submissions -> challenges (challenge));
diesel::joinable!(submissions -> users (user));

diesel::allow_tables_to_appear_in_same_query!(
    artifacts,
    challenges,
    problemsets,
    solved,
    submissions,
    users,
);
