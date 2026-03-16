# API Documentation

Generate an API specification for developers consuming this API.

## Sections

### 1. Overview

Table with columns: Property | Value

Include: base URL, content type, date format, API version, framework.

### 2. Authentication

Table with columns: Method | Header | Description

Describe how clients authenticate. Omit if no auth is found.

### 3. Endpoints

Group endpoints by resource (the URL noun, e.g., "Users", "Orders").

Example grouping:

```
### Users
#### GET /api/users
#### POST /api/users
#### GET /api/users/:id
```

For each endpoint:

- **Method and path**: e.g., `GET /api/users/:id`
- **Description**: what the endpoint does
- **Request**: table of fields with Type and Required columns
- **Response**: table of fields with Type column
- **Status codes**: table of Code and Description

### 4. Pagination

If list endpoints use pagination, document:

- Query parameters (page, limit, cursor, etc.)
- Response envelope structure (data, meta, next_cursor, etc.)

Omit if no paginated endpoints exist.

### 5. Error Format

Show the standard error response structure as a JSON example.
If rate-limit errors use a distinct format (e.g., 429 with Retry-After header), document them separately.

### 6. Types

Table with columns: Type | Source File | Fields | Description

List shared types, DTOs, and schemas used across endpoints.

## Analysis Techniques

1. **Framework detection**: check `package.json`, `requirements.txt`, `go.mod`, `Cargo.toml` to identify the web framework
2. **Route discovery**: use framework-specific patterns:
   - Next.js: `app/api/**/route.ts`
   - Express: `**/routes/**/*.ts`
   - FastAPI: Grep for `@app.get`, `@router.post` etc.
   - Generic fallback: `**/routes/**/*.{ts,js}`, `**/api/**/*.{ts,js}`
3. **Schema discovery**: Glob for `**/*.schema.ts`, `**/types.ts`, Prisma schemas, TypeORM entities
4. **Auth detection**: Grep for auth middleware, JWT patterns, `Authorization` header usage
5. **Route-schema correlation**: match route handler parameter types to schema definitions

## Writing Guidelines

- Write for a developer integrating with this API
- Include realistic example values where helpful
- Use `file_path:line_number` references for source locations
- When updating, verify each `file_path:line_number` reference is still accurate

## Omit Rules

- Omit Authentication if no auth mechanism is found
- Omit Pagination if no paginated endpoints exist
- Keep all other sections — an API doc without Endpoints or Error Format is incomplete
