-- Test database initialization script
-- This runs when the test PostgreSQL container starts

-- Create test schema
CREATE SCHEMA IF NOT EXISTS test_schema;

-- Users table with various data types
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE,
    active BOOLEAN DEFAULT true,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Orders table with foreign key
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    amount NUMERIC(10,2) NOT NULL,
    status VARCHAR(50) DEFAULT 'pending',
    notes TEXT,
    order_date DATE DEFAULT CURRENT_DATE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Products table for testing more types
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price NUMERIC(10,2),
    weight REAL,
    dimensions DOUBLE PRECISION[],
    tags TEXT[],
    metadata JSONB,
    image_data BYTEA,
    product_uuid UUID DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Table in test_schema for schema testing
CREATE TABLE test_schema.settings (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert test data for users
INSERT INTO users (name, email, active, metadata) VALUES
    ('Alice Smith', 'alice@example.com', true, '{"role": "admin", "permissions": ["read", "write", "delete"]}'),
    ('Bob Jones', 'bob@example.com', true, '{"role": "user", "permissions": ["read"]}'),
    ('Charlie Brown', 'charlie@example.com', false, '{"role": "user", "suspended": true}'),
    ('Diana Prince', 'diana@example.com', true, NULL);

-- Insert test data for orders
INSERT INTO orders (user_id, amount, status, notes) VALUES
    (1, 99.99, 'completed', 'First order'),
    (1, 149.50, 'pending', NULL),
    (2, 25.00, 'completed', 'Small order'),
    (2, 500.00, 'shipped', 'Large order with multiple items'),
    (3, 75.25, 'cancelled', 'Customer requested cancellation');

-- Insert test data for products
INSERT INTO products (name, description, price, weight, tags, metadata) VALUES
    ('Widget Pro', 'Professional-grade widget', 29.99, 0.5, ARRAY['electronics', 'tools'], '{"brand": "ACME", "warranty_years": 2}'),
    ('Gadget X', 'Next-generation gadget', 199.99, 1.2, ARRAY['electronics', 'premium'], '{"brand": "TechCorp", "color": "black"}'),
    ('Basic Tool', 'Simple but reliable', 9.99, 0.25, ARRAY['tools', 'budget'], NULL);

-- Insert test data for test_schema settings
INSERT INTO test_schema.settings (key, value) VALUES
    ('app_name', 'vizgres'),
    ('version', '0.1.0'),
    ('debug_mode', 'false');

-- Create indexes for testing
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_status_date ON orders(status, order_date);

-- View for testing
CREATE VIEW user_order_summary AS
SELECT
    u.id,
    u.name,
    COUNT(o.id) as order_count,
    COALESCE(SUM(o.amount), 0) as total_amount
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name;

-- Materialized view for testing (shows up alongside views)
CREATE MATERIALIZED VIEW active_user_stats AS
SELECT
    u.id,
    u.name,
    u.email,
    COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.active = true
GROUP BY u.id, u.name, u.email;

-- Functions for testing
CREATE FUNCTION get_user_by_id(user_id integer)
RETURNS SETOF users
LANGUAGE sql STABLE
AS $$
    SELECT * FROM users WHERE id = user_id;
$$;

CREATE FUNCTION calculate_order_total(p_user_id integer)
RETURNS numeric
LANGUAGE sql STABLE
AS $$
    SELECT COALESCE(SUM(amount), 0) FROM orders WHERE user_id = p_user_id;
$$;

CREATE FUNCTION format_user_name(first_name text, last_name text)
RETURNS text
LANGUAGE sql IMMUTABLE
AS $$
    SELECT first_name || ' ' || last_name;
$$;

-- Procedure for testing (PostgreSQL 11+)
CREATE PROCEDURE archive_old_orders(cutoff_date date)
LANGUAGE sql
AS $$
    UPDATE orders SET status = 'archived' WHERE order_date < cutoff_date AND status = 'completed';
$$;
