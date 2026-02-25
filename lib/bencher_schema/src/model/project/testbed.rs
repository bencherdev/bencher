use bencher_json::{
    DateTime, JsonNewTestbed, JsonTestbed, NameId, ResourceName, TestbedNameId, TestbedSlug,
    TestbedUuid,
    project::testbed::{JsonTestbedPatch, JsonUpdateTestbed},
};
#[cfg(feature = "plus")]
use bencher_json::{JsonSpec, SpecResourceId, project::testbed::JsonTestbedPatchNull};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{ProjectId, QueryProject};
#[cfg(feature = "plus")]
use crate::error::bad_request_error;
#[cfg(feature = "plus")]
use crate::model::spec::QuerySpec;
use crate::model::spec::SpecId;
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{BencherResource, assert_parentage, resource_conflict_err},
    macros::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
    schema::{self, testbed as testbed_table},
    write_conn,
};

crate::macros::typed_id::typed_id!(TestbedId);

/// Resolved testbed and optional spec for report creation.
pub struct ResolvedTestbed {
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
}

/// Whether the testbed was explicitly specified by the user or derived from context.
pub enum RunTestbed {
    /// User explicitly provided `--testbed`.
    Explicit,
    /// Testbed was derived from context (OS name) or defaulted.
    Derived,
}

/// Whether a job was requested and how its spec should be resolved.
#[cfg(feature = "plus")]
pub enum RunJob<'a> {
    /// No job requested — skip the job resolution path entirely.
    None,
    /// Job requested with an explicit `--spec`.
    WithSpec(&'a SpecResourceId),
    /// Job requested without an explicit spec — resolve from testbed or fallback.
    WithoutSpec,
}

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = testbed_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryTestbed {
    pub id: TestbedId,
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub slug: TestbedSlug,
    pub spec_id: Option<SpecId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryTestbed {
    fn_eq_resource_id!(testbed, TestbedResourceId);
    fn_from_resource_id!(project_id, ProjectId, testbed, Testbed, TestbedResourceId);

    fn_eq_name_id!(ResourceName, testbed, TestbedNameId);
    fn_from_name_id!(testbed, Testbed, TestbedNameId);

    fn_get!(testbed, TestbedId);
    fn_get_id!(testbed, TestbedId, TestbedUuid);
    fn_get_uuid!(testbed, TestbedId, TestbedUuid);
    fn_from_uuid!(project_id, ProjectId, testbed, TestbedUuid, Testbed);

    /// Get or create a testbed for a report.
    ///
    /// When no job is requested, this is a simple get-or-create.
    /// When a job is requested, spec resolution follows this order:
    /// 1. Explicit `--spec` → use it; derive testbed name from spec for `Derived`
    /// 2. Explicit testbed exists with `spec_id` → use that spec
    /// 3. Fallback spec → use it; derive testbed name from spec for `Derived`
    /// 4. Error if no spec resolvable
    pub async fn get_or_create(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &TestbedNameId,
        run_testbed: &RunTestbed,
        #[cfg(feature = "plus")] run_job: &RunJob<'_>,
    ) -> Result<ResolvedTestbed, HttpError> {
        #[cfg(feature = "plus")]
        if matches!(run_job, RunJob::WithSpec(_) | RunJob::WithoutSpec) {
            return Self::get_or_create_for_job(context, project_id, testbed, run_testbed, run_job)
                .await;
        }

        let query_testbed = Self::get_or_create_for_report(context, project_id, testbed).await?;
        Ok(ResolvedTestbed {
            testbed_id: query_testbed.id,
            spec_id: None,
        })
    }

    async fn get_or_create_for_report(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &TestbedNameId,
    ) -> Result<Self, HttpError> {
        let query_testbed = Self::get_or_create_inner(context, project_id, testbed).await?;

        if query_testbed.archived.is_some() {
            let update_testbed = UpdateTestbed::unarchive();
            diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(query_testbed.id)))
                .set(&update_testbed)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Testbed, &query_testbed))?;
        }

        Ok(query_testbed)
    }

    /// Get or create a testbed for a job run, resolving the spec and
    /// potentially deriving the testbed name from the spec.
    #[cfg(feature = "plus")]
    async fn get_or_create_for_job(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &TestbedNameId,
        run_testbed: &RunTestbed,
        run_job: &RunJob<'_>,
    ) -> Result<ResolvedTestbed, HttpError> {
        // 1. Explicit --spec
        if let RunJob::WithSpec(spec) = run_job {
            let query_spec = QuerySpec::from_active_resource_id(auth_conn!(context), spec)?;
            let testbed = match run_testbed {
                RunTestbed::Explicit => testbed.clone(),
                RunTestbed::Derived => TestbedNameId::new_name(query_spec.name.clone()),
            };
            let query_testbed =
                Self::get_or_create_for_report(context, project_id, &testbed).await?;
            Self::maybe_assign_spec(context, &query_testbed, query_spec.id).await?;
            return Ok(ResolvedTestbed {
                testbed_id: query_testbed.id,
                spec_id: Some(query_spec.id),
            });
        }

        // 2. Explicit testbed that already exists with a spec
        if matches!(run_testbed, RunTestbed::Explicit)
            && let Ok(query_testbed) = Self::from_name_id(auth_conn!(context), project_id, testbed)
            && let Some(spec_id) = query_testbed.spec_id
        {
            if query_testbed.archived.is_some() {
                let update_testbed = UpdateTestbed::unarchive();
                diesel::update(
                    schema::testbed::table.filter(schema::testbed::id.eq(query_testbed.id)),
                )
                .set(&update_testbed)
                .execute(write_conn!(context))
                .map_err(resource_conflict_err!(Testbed, &query_testbed))?;
            }
            return Ok(ResolvedTestbed {
                testbed_id: query_testbed.id,
                spec_id: Some(spec_id),
            });
        }

        // 3. Fallback spec
        if let Some(query_spec) = QuerySpec::get_fallback(auth_conn!(context))? {
            let testbed = match run_testbed {
                RunTestbed::Explicit => testbed.clone(),
                RunTestbed::Derived => TestbedNameId::new_name(query_spec.name.clone()),
            };
            let query_testbed =
                Self::get_or_create_for_report(context, project_id, &testbed).await?;
            Self::maybe_assign_spec(context, &query_testbed, query_spec.id).await?;
            return Ok(ResolvedTestbed {
                testbed_id: query_testbed.id,
                spec_id: Some(query_spec.id),
            });
        }

        // 4. Error
        Err(bad_request_error(
            "No spec provided, no spec on testbed, and no fallback spec configured",
        ))
    }

    /// Assign a spec to a testbed only if it differs from the current spec.
    #[cfg(feature = "plus")]
    async fn maybe_assign_spec(
        context: &ApiContext,
        testbed: &Self,
        spec_id: SpecId,
    ) -> Result<(), HttpError> {
        if testbed.spec_id == Some(spec_id) {
            return Ok(());
        }
        diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed.id)))
            .set((
                schema::testbed::spec_id.eq(Some(spec_id)),
                schema::testbed::modified.eq(DateTime::now()),
            ))
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Testbed, testbed.id))?;
        Ok(())
    }

    async fn get_or_create_inner(
        context: &ApiContext,
        project_id: ProjectId,
        testbed: &TestbedNameId,
    ) -> Result<Self, HttpError> {
        let query_testbed = Self::from_name_id(auth_conn!(context), project_id, testbed);

        let http_error = match query_testbed {
            Ok(testbed) => return Ok(testbed),
            Err(e) => e,
        };

        let json_testbed = match testbed.clone() {
            NameId::Uuid(_) => return Err(http_error),
            NameId::Slug(slug) => JsonNewTestbed {
                name: slug.clone().into(),
                slug: Some(slug),
                #[cfg(feature = "plus")]
                spec: None,
            },
            NameId::Name(name) => JsonNewTestbed {
                name,
                slug: None,
                #[cfg(feature = "plus")]
                spec: None,
            },
        };

        Self::create(context, project_id, json_testbed).await
    }

    pub async fn create(
        context: &ApiContext,
        project_id: ProjectId,
        json_testbed: JsonNewTestbed,
    ) -> Result<Self, HttpError> {
        #[cfg(feature = "plus")]
        InsertTestbed::rate_limit(context, project_id).await?;

        let insert_testbed =
            InsertTestbed::from_json(auth_conn!(context), project_id, json_testbed)?;
        diesel::insert_into(schema::testbed::table)
            .values(&insert_testbed)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Testbed, insert_testbed))?;

        Self::from_uuid(auth_conn!(context), project_id, insert_testbed.uuid)
    }

    #[cfg(feature = "plus")]
    fn resolve_spec(
        conn: &mut DbConnection,
        spec_id: Option<SpecId>,
    ) -> Result<Option<JsonSpec>, HttpError> {
        spec_id
            .map(|id| QuerySpec::get(conn, id).map(QuerySpec::into_json))
            .transpose()
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonTestbed, HttpError> {
        let spec_id = self.spec_id;
        self.into_json_for_spec(conn, project, spec_id)
    }

    pub fn get_json_for_report(
        conn: &mut DbConnection,
        project: &QueryProject,
        testbed_id: TestbedId,
        spec_id: Option<SpecId>,
    ) -> Result<JsonTestbed, HttpError> {
        let testbed = Self::get(conn, testbed_id)?;
        testbed.into_json_for_spec(conn, project, spec_id)
    }

    pub fn into_json_for_spec(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
        spec_id: Option<SpecId>,
    ) -> Result<JsonTestbed, HttpError> {
        #[cfg(not(feature = "plus"))]
        let _ = (conn, spec_id);
        #[cfg(feature = "plus")]
        let spec = Self::resolve_spec(conn, spec_id)?;
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            archived,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Testbed,
            project_id,
        );
        Ok(JsonTestbed {
            uuid,
            project: project.uuid,
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
            created,
            modified,
            archived,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = testbed_table)]
pub struct InsertTestbed {
    pub uuid: TestbedUuid,
    pub project_id: ProjectId,
    pub name: ResourceName,
    pub slug: TestbedSlug,
    pub spec_id: Option<SpecId>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertTestbed {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(testbed, Testbed);

    fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        testbed: JsonNewTestbed,
    ) -> Result<Self, HttpError> {
        let JsonNewTestbed {
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
        } = testbed;
        let slug = ok_slug!(conn, project_id, &name, slug, testbed, QueryTestbed);
        #[cfg(feature = "plus")]
        let spec_id = spec
            .as_ref()
            .map(|resource_id| {
                QuerySpec::from_resource_id(conn, resource_id).map(|query_spec| query_spec.id)
            })
            .transpose()?;
        #[cfg(not(feature = "plus"))]
        let spec_id = None;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: TestbedUuid::new(),
            project_id,
            name,
            slug,
            spec_id,
            created: timestamp,
            modified: timestamp,
            archived: None,
        })
    }

    #[expect(
        clippy::expect_used,
        reason = "localhost has no spec, so from_json cannot fail"
    )]
    pub fn localhost(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewTestbed::localhost())
            .expect("Failed to create localhost testbed")
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = testbed_table)]
pub struct UpdateTestbed {
    pub name: Option<ResourceName>,
    pub slug: Option<TestbedSlug>,
    pub spec_id: Option<Option<SpecId>>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

impl UpdateTestbed {
    pub fn from_json(conn: &mut DbConnection, json: JsonUpdateTestbed) -> Result<Self, HttpError> {
        match json {
            JsonUpdateTestbed::Patch(patch) => {
                let JsonTestbedPatch {
                    name,
                    slug,
                    #[cfg(feature = "plus")]
                    spec,
                    archived,
                } = patch;
                #[cfg(feature = "plus")]
                let spec_id = spec
                    .map(|resource_id| {
                        QuerySpec::from_resource_id(conn, &resource_id)
                            .map(|query_spec| Some(query_spec.id))
                    })
                    .transpose()?;
                #[cfg(not(feature = "plus"))]
                let spec_id = {
                    let _ = conn;
                    None
                };
                let modified = DateTime::now();
                let archived = archived.map(|archived| archived.then_some(modified));
                Ok(Self {
                    name,
                    slug,
                    spec_id,
                    modified,
                    archived,
                })
            },
            #[cfg(feature = "plus")]
            JsonUpdateTestbed::Null(patch_null) => {
                let JsonTestbedPatchNull {
                    name,
                    slug,
                    spec: (),
                    archived,
                } = patch_null;
                let modified = DateTime::now();
                let archived = archived.map(|archived| archived.then_some(modified));
                Ok(Self {
                    name,
                    slug,
                    spec_id: Some(None),
                    modified,
                    archived,
                })
            },
            #[cfg(not(feature = "plus"))]
            #[expect(
                clippy::unreachable,
                reason = "Null variant is only constructed with the plus feature"
            )]
            JsonUpdateTestbed::Null(_) => unreachable!(),
        }
    }

    fn unarchive() -> Self {
        let modified = DateTime::now();
        Self {
            name: None,
            slug: None,
            spec_id: None,
            modified,
            archived: Some(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use crate::{
        schema,
        test_util::{
            CreateSpecArgs, clear_testbed_spec, create_base_entities, create_spec, create_testbed,
            delete_spec, get_testbed_spec_id, set_testbed_spec, setup_test_db,
        },
    };

    #[test]
    fn testbed_created_without_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), None);
    }

    #[test]
    fn testbed_created_with_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        set_testbed_spec(&mut conn, testbed_id, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_id));
    }

    #[test]
    fn testbed_assign_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), None);

        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        set_testbed_spec(&mut conn, testbed_id, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_id));
    }

    #[test]
    fn testbed_change_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_a = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let spec_b = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000b2",
                name: "Spec B",
                slug: "spec-b",
                architecture: "aarch64",
                cpu: 8,
                memory: 0x0004_0000_0000,
                disk: 214_748_364_800,
                network: true,
            },
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        set_testbed_spec(&mut conn, testbed_id, spec_a);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_a));

        set_testbed_spec(&mut conn, testbed_id, spec_b);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_b));
    }

    #[test]
    fn testbed_clear_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        set_testbed_spec(&mut conn, testbed_id, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_id));

        clear_testbed_spec(&mut conn, testbed_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), None);
    }

    #[test]
    fn multiple_testbeds_share_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Shared Spec",
                slug: "shared-spec",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_a = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Testbed A",
            "testbed-a",
        );
        let testbed_b = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "Testbed B",
            "testbed-b",
        );
        set_testbed_spec(&mut conn, testbed_a, spec_id);
        set_testbed_spec(&mut conn, testbed_b, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_a), Some(spec_id));
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_b), Some(spec_id));
    }

    #[test]
    fn testbed_spec_on_delete_set_null() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        set_testbed_spec(&mut conn, testbed_id, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_id));

        delete_spec(&mut conn, spec_id);
        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), None);
    }

    #[test]
    fn testbed_update_preserves_spec() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Test Testbed",
            "test-testbed",
        );
        set_testbed_spec(&mut conn, testbed_id, spec_id);

        // Update just the name — spec should remain
        diesel::update(schema::testbed::table.filter(schema::testbed::id.eq(testbed_id)))
            .set(schema::testbed::name.eq("Renamed Testbed"))
            .execute(&mut conn)
            .expect("Failed to update testbed name");

        assert_eq!(get_testbed_spec_id(&mut conn, testbed_id), Some(spec_id));
    }

    #[test]
    fn testbed_spec_query() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let spec_id = create_spec(
            &mut conn,
            CreateSpecArgs {
                uuid: "00000000-0000-0000-0000-0000000000a1",
                name: "Spec A",
                slug: "spec-a",
                architecture: "x86_64",
                cpu: 4,
                memory: 0x0002_0000_0000,
                disk: 107_374_182_400,
                network: false,
            },
        );
        let testbed_with = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "Testbed With Spec",
            "testbed-with",
        );
        let _testbed_without = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "Testbed Without Spec",
            "testbed-without",
        );
        set_testbed_spec(&mut conn, testbed_with, spec_id);

        let with_spec: Vec<i32> = schema::testbed::table
            .filter(schema::testbed::spec_id.eq(spec_id))
            .select(schema::testbed::id)
            .load(&mut conn)
            .expect("Failed to query testbeds by spec_id");
        assert_eq!(with_spec, vec![testbed_with]);

        let without_spec: Vec<i32> = schema::testbed::table
            .filter(schema::testbed::spec_id.is_null())
            .select(schema::testbed::id)
            .load(&mut conn)
            .expect("Failed to query testbeds without spec_id");
        assert_eq!(without_spec.len(), 1);
    }
}
