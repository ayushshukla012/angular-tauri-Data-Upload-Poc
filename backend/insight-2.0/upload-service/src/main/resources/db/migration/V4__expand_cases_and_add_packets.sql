ALTER TABLE cases RENAME COLUMN phone_number TO mobile_number;

ALTER TABLE cases
    ADD COLUMN reference_number VARCHAR(128),
    ADD COLUMN date_of_birth VARCHAR(16),
    ADD COLUMN address TEXT,
    ADD COLUMN state_ut_code VARCHAR(8),
    ADD COLUMN pincode VARCHAR(10),
    ADD COLUMN email VARCHAR(256),
    ADD COLUMN information_fy VARCHAR(16),
    ADD COLUMN information_source_type VARCHAR(128),
    ADD COLUMN information_source_description TEXT,
    ADD COLUMN information_type VARCHAR(128),
    ADD COLUMN information_description TEXT,
    ADD COLUMN information_value VARCHAR(256),
    ADD COLUMN nature_of_verification VARCHAR(128),
    ADD COLUMN actionable_ay VARCHAR(16),
    ADD COLUMN verification_result_type_1 VARCHAR(128),
    ADD COLUMN verification_result_description_1 TEXT,
    ADD COLUMN verification_result_value_1 VARCHAR(256),
    ADD COLUMN verification_result_type_2 VARCHAR(128),
    ADD COLUMN verification_result_description_2 TEXT,
    ADD COLUMN verification_result_value_2 VARCHAR(256),
    ADD COLUMN verification_result_type_3 VARCHAR(128),
    ADD COLUMN verification_result_description_3 TEXT,
    ADD COLUMN verification_result_value_3 VARCHAR(256),
    ADD COLUMN remarks TEXT,
    ADD COLUMN batch_number VARCHAR(64);

CREATE TABLE packets (
    batch_number              VARCHAR(64) PRIMARY KEY,
    description                TEXT,
    submitting_person_name     VARCHAR(256) NOT NULL,
    submitting_person_address  TEXT,
    submitting_person_mobile   VARCHAR(20)  NOT NULL,
    submitting_person_email    VARCHAR(256),
    created_at                 TIMESTAMPTZ  NOT NULL
);

ALTER TABLE cases
    ADD CONSTRAINT fk_cases_batch_number FOREIGN KEY (batch_number) REFERENCES packets (batch_number);

CREATE INDEX idx_cases_batch_number ON cases (batch_number);
