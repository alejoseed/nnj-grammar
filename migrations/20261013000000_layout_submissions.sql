CREATE TABLE class_codes (
    code text PRIMARY KEY CHECK (code = upper(code)),
    label text NOT NULL DEFAULT '',
    expires_at timestamptz NOT NULL
);

CREATE TABLE layout_submissions (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    class_code text NOT NULL REFERENCES class_codes (code),
    nickname text,
    client_hash text NOT NULL,
    device_hash text NOT NULL,
    sentence text NOT NULL,
    compact boolean NOT NULL,
    settings jsonb,
    document jsonb NOT NULL,
    moved_nodes integer NOT NULL,
    moved_labels integer NOT NULL,
    bent_links integer NOT NULL,
    displacement_px double precision NOT NULL,
    order_breaks integer NOT NULL,
    status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'rejected')),
    review_note text
);

CREATE INDEX layout_submissions_sentence ON layout_submissions (sentence, status);
CREATE INDEX layout_submissions_class ON layout_submissions (class_code, created_at);
CREATE INDEX layout_submissions_device ON layout_submissions (device_hash);
