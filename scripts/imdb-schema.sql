-- IMDb Non-Commercial Dataset schema
-- Source: https://developer.imdb.com/non-commercial-datasets/
-- License: Personal and non-commercial use only

CREATE TABLE IF NOT EXISTS title_basics (
    tconst          text PRIMARY KEY,
    title_type      text,
    primary_title   text,
    original_title  text,
    is_adult        smallint,
    start_year      smallint,
    end_year        smallint,
    runtime_minutes integer,
    genres          text
);

CREATE TABLE IF NOT EXISTS title_akas (
    title_id        text NOT NULL,
    ordering        integer NOT NULL,
    title           text,
    region          text,
    language        text,
    types           text,
    attributes      text,
    is_original     smallint,
    PRIMARY KEY (title_id, ordering)
);

CREATE TABLE IF NOT EXISTS title_crew (
    tconst      text PRIMARY KEY,
    directors   text,
    writers     text
);

CREATE TABLE IF NOT EXISTS title_episode (
    tconst          text PRIMARY KEY,
    parent_tconst   text,
    season_number   integer,
    episode_number  integer
);

CREATE TABLE IF NOT EXISTS title_principals (
    tconst      text NOT NULL,
    ordering    integer NOT NULL,
    nconst      text,
    category    text,
    job         text,
    characters  text,
    PRIMARY KEY (tconst, ordering)
);

CREATE TABLE IF NOT EXISTS title_ratings (
    tconst          text PRIMARY KEY,
    average_rating  numeric(3,1),
    num_votes       integer
);

CREATE TABLE IF NOT EXISTS name_basics (
    nconst              text PRIMARY KEY,
    primary_name        text,
    birth_year          smallint,
    death_year          smallint,
    primary_profession  text,
    known_for_titles    text
);
