## Getting Started

In order to get started with the project, you will need to have the following
prerequisites installed on your machine.

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/en/)
- [typeshare](https://crates.io/crates/typeshare)
- [wasm-pack](https://crates.io/crates/wasm-pack)

### Installation

1. Fork the repository (optional)
2. Clone the repository
   ```shell
   git clone git@github.com:bencherdev/bencher.git
    ```
3. Checkout the `devel` branch
   ```shell
   git checkout devel
   ```
4. Build the project
    ```shell
    cargo build
    ```
5. Run the API service
   ```shell
   cd services/api
   cargo run
   ```
6. Run the Console
    ```shell
    cd services/console
    npm run dev
    ```
7. Configure the CLI environment
   ```shell
   cd ./services/cli 
   . ./env.sh
   ```

### Accessing the Console

Once the application is running, you can access it by visiting
[http://localhost:3000](http://localhost:3000) in your browser.

1. Sign up for an account by entering your name and email address.
2. Click on the "Confirm Email" link that is provided in the API logs.

### Accessing the API

The API is accessible at [http://localhost:61016](http://localhost:61016).

### Seeding the Database

To seed the database with sample data, run the following command:

```shell
cargo test-api seed
```

### Accessing the Database

The data is stored in a SQLite database that is created in the `services/api/data` directory
when the application is started.  The database can be accessed using the `sqlite3` command:

```shell
sqlite3 services/api/data/bencher.db
```

#### Entity Relationship Diagram

```mermaid
erDiagram
    alert {
        id INTEGER
        uuid TEXT
        boundary_id INTEGER
        boundary_limit BOOLEAN
        status INTEGER
        modified BIGINT
    }
    benchmark {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        name TEXT
        slug TEXT
        created BIGINT
        modified BIGINT
        archived BIGINT
    }
    boundary {
        id INTEGER
        uuid TEXT
        metric_id INTEGER
        threshold_id INTEGER
        model_id BOOLEAN
        baseline DOUBLE
        lower_limit DOUBLE
        upper_limit DOUBLE
    }
    branch {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        name TEXT
        slug TEXT
        head_id INTEGER
        created BIGINT
        modified BIGINT
        archived BIGINT
    }
    head {
        id INTEGER
        uuid TEXT
        branch_id INTEGER
        start_point_id INTEGER
        created BIGINT
        replaced BIGINT
    }
    head_version {
        id INTEGER
        head_id INTEGER
        version_id INTEGER
    }
    measure {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        name TEXT
        slug TEXT
        unites TEXT
        created BIGINT
        modified BIGINT
        archived BIGINT
    }
    metric {
        id INTEGER
        uuid TEXT
        report_benchmark_id INTEGER
        measure_id INTEGER
        value DOUBLE
        lower_value DOUBLE
        upper_value DOUBLE
    }
    model {
        id INTEGER
        uuid TEXT
        threshold_id INTEGER
        test INTEGER
        min_sample_size BIGINT
        max_sample_size BIGINT
        window BIGINT
        lower_boundary DOUBLE
        upper_boundary DOUBLE
        created BIGINT
        replaced BIGINT
    }
    organization {
        id INTEGER
        uuid TEXT
        name TEXT
        slug TEXT
        license TEXT
        created BIGINT
        modified BIGINT
    }
    organization_role {
        id INTEGER
        user_id INTEGER
        organization_id INTEGER
        role TEXT
        created BIGINT
        modified BIGINT
    }
    plan {
        id INTEGER
        organization_id INTEGER
        metered_plan TEXT
        licensed_plan TEXT
        license TEXT
        created BIGINT
        modified BIGINT
    }
    plot {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        rank BIGINT
        title TEXT
        lower_value BOOLEAN
        upper_value BOOLEAN
        lower_boundary BOOLEAN
        upper_boundary BOOLEAN
        x_axis INTEGER
        window BIGINT
        created BIGINT
        modified BIGINT
    }
    plot_benchmark {
        plot_id INTEGER
        benchmark_id INTEGER
        rank BIGINT
    }
    plot_branch {
        plot_id INTEGER
        branch_id INTEGER
        rank BIGINT
    }
    plot_measure {
        plot_id INTEGER
        measure_id INTEGER
        rank BIGINT
    }
    plot_testbed {
        plot_id INTEGER
        testbed_id INTEGER
        rank BIGINT
    }
    project {
        id INTEGER
        uuid TEXT
        organization_id INTEGER
        name TEXT
        slug TEXT
        url TEXT
        visibility INTEGER
        created BIGINT
        modified BIGINT
    }
    project_role {
        id INTEGER
        user_id INTEGER
        project_id INTEGER
        role TEXT
        created BIGINT
        modified BIGINT
    }
    report {
        id INTEGER
        uuid TEXT
        user_id INTEGER
        project_id INTEGER
        head_id INTEGER
        version_id INTEGER
        testbed_id INTEGER
        adapter INTEGER
        start_time BIGINT
        end_time BIGINT
        created BIGINT
    }
    report_benchmark {
        id INTEGER
        uuid TEXT
        report_id INTEGER
        iteration INTEGER
        benchmark_id INTEGER
    }
    server {
        id INTEGER
        uuid TEXT
        created BIGINT
    }
    testbed {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        name TEXT
        slug TEXT
        created BIGINT
        modified BIGINT
        archived BIGINT
    }
    threshold {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        branch_id INTEGER
        testbed_id INTEGER
        measure_id INTEGER
        model_id INTEGER
        created BIGINT
        modified BIGINT
    }
    token {
        id INTEGER
        uuid TEXT
        user_id INTEGER
        name TEXT
        jwt TEXT
        creation BIGINT
        expiration BIGINT
    }
    user {
        id INTEGER
        uuid TEXT
        name TEXT
        slug TEXT
        email TEXT
        admin BOOLEAN
        locked BOOLEAN
        created BIGINT
        modified BIGINT
    }
    version {
        id INTEGER
        uuid TEXT
        project_id INTEGER
        number INTEGER
        hash TEXT
    }
 
    user ||--o{ token : "has"
    user ||--o{ organization_role : "has"
    user ||--o{ project_role : "has"
    user ||--o{ report : "writes"
    
    organization ||--o{ organization_role : "assigns"
    organization ||--o{ project : "owns"
    organization ||--o{ plan : "has"
    
    project ||--o{ project_role : "assigns"
    project ||--o{ version : "has"
    project ||--o{ threshold : "has"
    project ||--o{ testbed : "has"
    project ||--o{ benchmark : "has"
    project ||--o{ measure : "has"
    project ||--o{ branch : "has"
    project ||--o{ plot : "has"
    
    threshold ||--o{ model : "uses"
    threshold ||--o{ metric : "relates"
    
    benchmark ||--o{ plot_benchmark : "part of"
    
    measure ||--o{ plot_measure : "visualized"
    measure ||--o{ metric : "measured"
    measure ||--o{ threshold : "used"
    
    testbed ||--o{ plot_testbed : "analyzed"
    testbed ||--o{ report : "used in"
    
    branch ||--o{ plot_branch : "analyzed"
    branch ||--o{ head : "tracks"
    
    head ||--o{ head_version : "links"
    head_version ||--o{ version : "includes"
    
    report ||--o{ metric : "generates"
    report ||--o{ plot : "visualizes"
    report ||--o{ version : "reports on"
    report ||--o{ testbed : "runs on"
    
    alert ||--o{ metric : "monitors"
    metric ||--o{ report_benchmark : "derived from"
    
    server ||--o{ report : "hosts"
    
    plot ||--o{ plot_branch : "uses"
    plot ||--o{ plot_testbed : "uses"
    plot ||--o{ plot_benchmark : "uses"
    plot ||--o{ plot_measure : "uses"
```

### Testing with Docker

To test the application using Docker, follow these steps:

1. Build and run the project
   ```shell
   docker/run.sh
   ```
   or run directly with docker compose using the following command:

   **X64**
   ```shell
   ARCH=amd64 docker compose up --file docker/docker-compose.yml --build
   ```
   **ARM64**
   ```shell
   ARCH=arm64 docker compose up --file docker/docker-compose.yml --build
   ```
2. Open the console in your browser [http://localhost:3000](http://localhost:3000).
3. Sign up for an account by entering your name and email address.
4. Click on the "Confirm Email" link that is provided in the docker compose logs.
