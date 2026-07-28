package main

import (
	"context"
	"database/sql"
	"fmt"
	"time"

	_ "github.com/lib/pq"
)

type wikiStore struct {
	db *sql.DB
}

func openWikiStore(databaseURL string) (*wikiStore, error) {
	db, err := sql.Open("postgres", databaseURL)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(10)
	db.SetMaxIdleConns(5)
	db.SetConnMaxLifetime(time.Hour)

	store := &wikiStore{db: db}
	if err := store.createTable(); err != nil {
		_ = db.Close()
		return nil, err
	}
	return store, nil
}

func (s *wikiStore) createTable() error {
	const query = `
	CREATE TABLE IF NOT EXISTS group_wiki_repos (
		group_id   BIGINT PRIMARY KEY,
		owner      TEXT NOT NULL,
		repo       TEXT NOT NULL,
		updated_by BIGINT,
		updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
	);
	`
	_, err := s.db.Exec(query)
	return err
}

func (s *wikiStore) Close() error {
	if s == nil || s.db == nil {
		return nil
	}
	return s.db.Close()
}

func (s *wikiStore) SetGroupRepo(ctx context.Context, groupID int64, owner, repo string, updatedBy int64) error {
	if s == nil || s.db == nil {
		return fmt.Errorf("database not connected")
	}
	const query = `
	INSERT INTO group_wiki_repos (group_id, owner, repo, updated_by, updated_at)
	VALUES ($1, $2, $3, $4, NOW())
	ON CONFLICT (group_id) DO UPDATE SET
		owner = EXCLUDED.owner,
		repo = EXCLUDED.repo,
		updated_by = EXCLUDED.updated_by,
		updated_at = NOW();
	`
	_, err := s.db.ExecContext(ctx, query, groupID, owner, repo, updatedBy)
	return err
}

func (s *wikiStore) GetGroupRepo(ctx context.Context, groupID int64) (owner, repo string, found bool, err error) {
	if s == nil || s.db == nil {
		return "", "", false, fmt.Errorf("database not connected")
	}
	const query = `SELECT owner, repo FROM group_wiki_repos WHERE group_id = $1`
	err = s.db.QueryRowContext(ctx, query, groupID).Scan(&owner, &repo)
	if err == sql.ErrNoRows {
		return "", "", false, nil
	}
	if err != nil {
		return "", "", false, err
	}
	return owner, repo, true, nil
}
