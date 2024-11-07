-- Your SQL goes here

CREATE TABLE "new_users" (
	"id"	INTEGER,
	"username"	TEXT NOT NULL UNIQUE,
	"password"	TEXT NOT NULL,
	"email"	TEXT NOT NULL,
	"contact"	TEXT NOT NULL,
	"enabled"	BOOLEAN NOT NULL,
	"role"	TEXT NOT NULL,
    "nickname"  TEXT,
	PRIMARY KEY("id")
);

INSERT INTO "new_users" SELECT * FROM "users";
DROP TABLE "users";
ALTER TABLE "new_users" RENAME TO "users";
