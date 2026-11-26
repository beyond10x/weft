# Actor tables proposed by the domain authors (2026-11-26)

Input for the actors pass (`ess/9-actors/actors.yaml`, domain `weft.actors`). Each line: actor ->
commands. "Views" means read access to that domain's views. Actor list: `docs/domain-sketch.md`
§ Structure, Actors. Decisions by the integrator are marked "decided".

## Human and client actors

- ClusterOperator: runs the cloud itself; never reads tenant secrets or workload data.
- TenantOwner, AccountAdmin, Deployer, Viewer: roles in an account (`weft.shared.Role`: owner,
  admin, deployer, viewer, billing).
- Billing: reads usage, quotas, prices, statements of its account (accounts report).
- Automation: an API token acting with one role in one account; never creates API tokens, never
  opens exec or attaches sessions.
- Anonymous: a CLI before login; may only start and collect a device login.

## System actors

- NodeAgent: a joined node; reports only about its own node.
- Scheduler: places instances, preempts, scales pools, drains idle nodes (decided: may scale and
  drain; the operator sets pool bounds).
- Translator: writes translations, coverage entries and translated formation rows.
- ReleaseController: runs release steps, marks releases live, records drift.
- ControlPlane: weft-control acting on its own: identity issue and expiry, secret delivery
  decisions, lease and session expiry, usage recording, statements, catalog reports.
- ManagementApi: weft-api acting on its own: login and token expiry, authorization records.
- Instance: a running workload through its workload identity.

## Per domain

- cluster: ClusterOperator -> every command except JoinNode, RecordNodeObservation; NodeAgent ->
  JoinNode (not yet joined), RecordNodeObservation (own node only); Scheduler ->
  RequestNodePoolScaleUp/Down, DrainNode (decided).
- inventory: ClusterOperator -> DefineResourceClass, RetireResourceClass, ResetDevice; NodeAgent ->
  ReportDevice, ReportDeviceWiped, ReportDeviceSliced, ReportDeviceFault, ReportDeviceRemoved.
- accounts: ClusterOperator -> CreateTenant, CloseTenant, GrantTopQuota, RaiseTopQuota,
  LowerTopQuota, BindTenantOwner; TenantOwner -> BindTenantOwner, CreateAccount, AddUser,
  AddServiceAccount, AddWorkloadPrincipal, DisablePrincipal, RevokeTenantOwner; TenantOwner and
  AccountAdmin -> CreateSubAccount, CloseAccount, BindRole, RevokeRole, AllocateQuota, DrawQuota,
  ReleaseQuota. Views: Tenants for ClusterOperator; the rest for TenantOwner, AccountAdmin, Viewer;
  QuotaUsage also Billing.
- access: ClusterOperator -> ConfigureAccess; Anonymous -> StartLogin, CollectLoginToken; every
  human -> ApproveLogin, DenyLogin, RevokeLogin (own login); TenantOwner, AccountAdmin, Deployer,
  Viewer -> CreateApiToken; TenantOwner, AccountAdmin -> RevokeApiToken; ManagementApi ->
  ExpireLogin, ExpireApiToken, RecordAuthorization.
- resources: Deployer, Automation, AccountAdmin -> create and delete Volume, Filesystem, Bucket,
  SnapshotVolume, DeleteVolumeSnapshot; NodeAgent -> the provisioned/taken reports.
- claims (moved from resources): ReleaseController, Deployer, Automation -> DeclareClaim,
  ReleaseClaim; Scheduler -> BindClaim, CommitBinding, CommitVolumeWriter; Scheduler, ControlPlane
  -> ReleaseBinding.
- artifacts: Deployer, Automation -> RegisterArtifact; TenantOwner, AccountAdmin -> RevokeArtifact
  (integrator default).
- formations: AccountAdmin -> CreateFormation, RetireFormation, SetFormationQuota,
  CreateEnvironment, RetireEnvironment, SetEnvironmentQuota; TenantOwner -> SetFormationQuota,
  SetEnvironmentQuota; Deployer, Automation -> CommitRevision and every add/remove/change of
  components, links, border, endpoints, routes, configs, mounts, overrides, scale settings;
  Translator -> CommitRevision, AddComponent, AddLink, AddToBorder, AddEndpoint, AddRoute,
  CreateConfig, MountConfig, SetConfigOverride; TenantOwner, AccountAdmin -> CreateExport,
  RevokeExport (moved from accounts).
- workloads: ClusterOperator -> RegisterRuntime, RetireRuntime; Deployer -> every user command
  (workloads, services, boxes, sessions, schedules, triggers, snapshots, warm pools), StopInstance,
  PauseInstance, ResumeInstance, OpenExec, WakeService (Explicit); Automation -> as Deployer
  except OpenExec and AttachSession; NodeAgent -> every Record* observation, CompleteInvocation,
  FailInvocation, RetryTask; Scheduler -> CreateInstance, ScheduleInstance, EvictInstance,
  StartJob, CompleteJob, FailJob, ExpireInvocation, AutoscaleService, BeginSleep, StartWaking,
  MissWake, RecordScheduleFired, WakeService (non-explicit); Instance -> Invoke, WakeService
  (LinkRequest).
- scheduling (PriorityClass and its commands moved to cluster, 2026-11-26): ControlPlane ->
  SubmitSchedulingRequest, RegisterGang, WithdrawSchedulingRequest, ReleasePlacement; Scheduler ->
  DeclareUnschedulable, RetrySchedulingRequest, WithdrawSchedulingRequest, PlaceInstance,
  BindPlacement, ReleasePlacement, BindGang, AbandonGang, PreemptPlacement, RequestScaleUp,
  RequestScaleDown, RecordScaleFulfilled, RecordScaleRefused; ReleaseController ->
  SubmitSchedulingRequest (integrator default: yes).
- nodes: NodeAgent -> OpenNodeSession, SupersedeNodeSession, Heartbeat, CloseNodeSession,
  RenewLease, InstanceObserved, NodeObserved, UsageReported, ExecEnded; Scheduler -> Assign,
  Stop, Pause, Resume, Snapshot, ReleaseAssignment, EndAssignment, Attach, Detach, CompleteDrain,
  Drain; ClusterOperator -> Drain, CancelDrain; ControlPlane -> LoseNodeSession; Deployer ->
  OpenExecSession, CloseExecSession; Viewer -> OpenExecSession of kind Logs only (role check is
  external).
- identity: ClusterOperator -> CreateTrustDomain, RotateSigningKey, FinishKeyRotation,
  RetireTrustDomain, RevokeIdentityDocument; NodeAgent -> IssueIdentityDocument,
  RenewIdentityDocument, RevokeIdentityDocument (instance ended); ControlPlane ->
  ExpireIdentityDocument; TenantOwner, AccountAdmin, Automation (admin role) ->
  CreateExchangePolicy, WithdrawExchangePolicy; Instance -> ExchangeToken.
- secrets: ClusterOperator -> RegisterSecretBackend; Deployer, AccountAdmin, Automation ->
  CreateSecret, AddSecretVersion, GrantSecret; plus TenantOwner -> DisableSecretVersion,
  RevokeSecretGrant; NodeAgent -> DeliverSecret. Views: Viewer and all account roles;
  ClusterOperator reads only SecretBackends.
- deployments: ClusterOperator -> RegisterTranslator, RetireTranslator; Deployer, Automation ->
  SubmitDeploymentSource, WithdrawDeploymentSource, TranslateSource, CreatePlan, PlanRollback,
  DiscardPlan, ApplyPlan; Translator -> RecordCoverageEntry, CompleteTranslation,
  RefuseTranslation, FailTranslation; ReleaseController -> RecordPlanResult, StartRelease,
  AddReleaseStep, StartReleaseStep, CompleteReleaseStep, FailReleaseStep, MarkReleaseLive,
  MarkRollbackLive, RecordDrift, ResolveDrift.
- catalog: ClusterOperator -> engines, offerings, plans, publishers, products and versions
  (register, publish, swap, retire, withdraw, move instance to engine); Deployer, AccountAdmin,
  Automation -> provision, change plan, set sleep policy, deprovision, bind, unbind, backup,
  restore, install, upgrade, uninstall, set installation sleep policy; Instance ->
  CheckServiceAccess; ControlPlane -> every Report*. Views: Viewer all except EngineList,
  OfferingEngines, ServiceInstancePlacement (ClusterOperator only).
- metering: ClusterOperator -> CreatePriceList, SetPrice, RemovePrice, PublishPriceList,
  DefinePricingModel, RetirePricingModel, AssignPricing, EndPricingAssignment; ControlPlane ->
  RecordUsage, ChargeBudget, IssueStatement; Scheduler -> AdmitWork, AcquireConcurrency,
  ReleaseConcurrency; TenantOwner, AccountAdmin, Deployer, Automation -> SetDeadline,
  SetConcurrency, SetRate; TenantOwner, AccountAdmin -> Change*/Remove* of those, SetBudget,
  ChangeBudget, RemoveBudget. Views: account roles in their own account; Billing; ClusterOperator
  also UsageByTenant, prices, pricing.
