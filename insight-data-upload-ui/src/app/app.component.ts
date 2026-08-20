import { ChangeDetectionStrategy, Component, OnDestroy, OnInit, computed, inject, signal, NgZone } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { AppConfigService } from './core/config/app-config.service';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ApiService } from './services/api.service';
import { DraftStoreService } from './services/draft-store.service';
import { ToastService } from './services/toast.service';
import { AttachedDocument, DraftState, InformationDetails, LoadedDraftBundle, PacketDetails, PersonRow, SavedDraftSummary, VerificationDetails, VerificationStatus } from './models';

interface NativePickedFile {
  file_name: string;
  file_path: string;
  file_size: number;
  extension: string;
  bytes: number[];
}

interface NativePickedCsvFile {
  fileName: string;
  filePath: string;
  fileSize: number;
}

interface NativePickedDocument {
  file_name: string;
  file_path: string;
  file_size: number;
  extension: string;
}

interface SelectedDocument {
  file: File;
  filePath?: string;
  fileSize: number;
}

interface CsvImportProgress {
  datasetId: string;
  importedCount: number;
  elapsedMs: number;
  rowsPerSecond: number;
  ready: boolean;
  completed: boolean;
}

interface CsvImportError {
  datasetId: string;
  message: string;
}

type ModalName = 'none' | 'addRow' | 'csv' | 'generalDoc' | 'openFile' | 'saveSuccess' | 'closeConfirm';
type PaginationItem = number | '…';

@Component({
  selector: 'idu-root',
  standalone: true,
  imports: [CommonModule, ReactiveFormsModule],
  templateUrl: './app.component.html',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class AppComponent implements OnInit, OnDestroy {
  private readonly fb = inject(FormBuilder);
  readonly api = inject(ApiService);
  readonly drafts = inject(DraftStoreService);
  readonly toast = inject(ToastService);
  readonly appConfig = inject(AppConfigService);
  private readonly ngZone = inject(NgZone);

  readonly step = signal(0);
  readonly modal = signal<ModalName>('none');
  readonly infoOpen = signal(false);
  readonly verificationOpen = signal(false);
  readonly personOpen = signal(true);
  readonly isOnline = signal(true);
  readonly importBusy = signal(false);
  readonly finalSubmitBusy = signal(false);
  readonly currentRowIndex = signal<number | null>(null);
  readonly currentDraftId = signal<string | null>(null);
  readonly selectedRows = signal<Set<string>>(new Set());
  readonly pageSize = signal(this.appConfig.value.pagination.defaultPageSize);
  readonly page = signal(1);
  readonly filter = signal('');
  readonly lastSavedAt = signal<string | null>(null);
  readonly progressMessage = signal('');
  readonly packetEditMode = signal(true);
  readonly documentType = signal('Verification Type');
  readonly documentDescription = signal('Found this digitally');
  readonly attachedForAll = signal(false);
  readonly csvFile = signal<File | null>(null);
  readonly csvNativePath = signal<string | null>(null);
  readonly generalDocumentFiles = signal<SelectedDocument[]>([]);
  readonly csvImportProgress = signal<string>('');
  readonly csvImportActive = signal(false);
  readonly csvImportReady = signal(false);
  readonly csvImportElapsedMs = signal<number | null>(null);
  private csvImportUnlisteners: UnlistenFn[] = [];
  readonly currentDocumentId = signal<string | null>(null);
  readonly savedDrafts = signal<SavedDraftSummary[]>([]);
  readonly storageLocation = signal<string>('');
  readonly openDraftBusy = signal(false);
  readonly nativeMode = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  readonly nativeRowStoreId = signal<string | null>(null);
  readonly nativeTotalRows = signal(0);
  readonly nativeFilteredTotalRows = signal(0);
  readonly nativeStoreDirty = signal(false);
  readonly zoomPercent = signal(100);
  readonly dirty = signal(false);
  readonly closing = signal(false);
  private restoringDraft = false;
  private closeUnlisten: (() => void) | null = null;
  private readonly tauriWindow = this.nativeMode ? getCurrentWindow() : null;

  readonly hasCompletedSelection = computed(() => {
    const selected = this.selectedRows();
    return this.rows().some(row => selected.has(row.caseId) && row.verificationStatus === 'Completed');
  });

  readonly packetForm = this.fb.group({
    reportType: ['', Validators.required],
    referenceNumber: ['', [Validators.required, Validators.maxLength(50)]],
    description: ['', [Validators.required, Validators.maxLength(2000)]],
    submittingPersonName: ['', Validators.required],
    submittingPersonMobile: ['', [Validators.required, Validators.pattern(/^[0-9]{10}$/)]],
    submittingPersonEmail: ['', [Validators.required, Validators.email]],
    submittingPersonAddress: ['', Validators.required]
  });

  readonly rowForm = this.fb.group({
    pan: ['', [Validators.required, Validators.pattern(/^[A-Za-z]{5}[0-9]{4}[A-Za-z]$/)]],
    name: ['', Validators.required],
    dobDoi: ['', Validators.required],
    mobile: ['', [Validators.required, Validators.pattern(/^[0-9]{10}$/)]],
    email: ['', [Validators.required, Validators.email]],
    pinCode: ['', [Validators.required, Validators.pattern(/^[0-9]{6}$/)]],
    address: ['', Validators.required],
    state: ['', Validators.required],
    verificationStatus: ['Pending' as VerificationStatus, Validators.required],

    informationFy: ['', Validators.required],
    informationSourceType: [''],
    informationSourceDescription: [''],
    informationType: ['', Validators.required],
    informationDescription: [''],
    informationValue: ['', Validators.required],
    source: ['', Validators.required],
    finding: ['', Validators.required],

    actionableAy: ['', Validators.required],
    statutoryReason: ['', Validators.required],
    verificationResultType: ['', Validators.required],
    incomeEscapingAssessmentValue: ['', Validators.required],
    verificationInformationValue: ['', Validators.required],
    resultDescription: ['']
  });

  readonly rows = signal<PersonRow[]>([]);
  readonly documents = signal<AttachedDocument[]>([]);

  readonly documentCountByCaseId = computed(() => {
    const counts = new Map<string, number>();
    for (const doc of this.documents()) {
      for (const caseId of doc.rowCaseIds) {
        counts.set(caseId, (counts.get(caseId) ?? 0) + 1);
      }
    }
    return counts;
  });

  readonly states = [
    'Andhra Pradesh', 'Arunachal Pradesh', 'Assam', 'Bihar', 'Chhattisgarh', 'Delhi', 'Goa', 'Gujarat',
    'Haryana', 'Himachal Pradesh', 'Jharkhand', 'Karnataka', 'Kerala', 'Madhya Pradesh', 'Maharashtra',
    'Odisha', 'Punjab', 'Rajasthan', 'Tamil Nadu', 'Telangana', 'Uttar Pradesh', 'Uttarakhand',
    'West Bengal'
  ];

  readonly reportTypes = ['Verification Report', 'Risk / Compliance Review', 'Other'];
  readonly years = ['2026-27', '2025-26', '2024-25', '2023-24', '2022-23'];
  readonly informationTypes = ['Financial Information', 'Property Information', 'Banking Information', 'Transaction Information', 'Other'];
  readonly sourceTypes = ['Digital', 'Physical', 'Portal', 'Third Party', 'Other'];
  readonly findings = ['Confirmed', 'Not Confirmed', 'Partially Confirmed', 'No Evidence', 'Other'];
  readonly verificationResults = ['Income Escaping Assessment', 'No Income Escaping', 'Further Verification Required', 'Other'];
  readonly statutoryReasons = ['Mismatch', 'Undisclosed Income', 'Unverified Information', 'Other'];

  readonly csvHeaders = [
    'PAN', 'Name', 'DOB/DOI', 'Mobile', 'E-Mail', 'PIN Code', 'Address', 'State',
    'FY', 'Information Type', 'Findings', 'Source', 'Information Value', 'Description',
    'Actionable AY', 'Verification Result Type', 'Statutory Reason',
    'Income Escaping Assessment Value', 'Verification Information Value'
  ];

  readonly filteredRows = computed(() => {
    const term = this.filter().trim().toLowerCase();
    const source = this.rows();
    if (this.nativeMode) return source;
    if (!term) return source;
    return source.filter(r => [r.pan, r.name, r.email, r.address, r.state, r.serialNo]
      .some(v => v.toLowerCase().includes(term)));
  });

  readonly pageCount = computed(() => this.nativeMode
    ? Math.max(1, Math.ceil(this.nativeFilteredTotalRows() / this.pageSize()))
    : Math.max(1, Math.ceil(this.filteredRows().length / this.pageSize())));

  readonly pagedRows = computed(() => {
    if (this.nativeMode) return this.rows();
    const filtered = this.filteredRows();
    const start = (this.page() - 1) * this.pageSize();
    return filtered.slice(start, start + this.pageSize());
  });

  readonly visibleRowCount = computed(() => this.nativeMode ? this.nativeFilteredTotalRows() : this.filteredRows().length);

  readonly hasDataOnScreen = computed(() => {
    const packet = this.packetForm.getRawValue();
    return this.nativeTotalRows() > 0 || this.rows().length > 0 || this.documents().length > 0 || Object.values(packet).some(value => String(value ?? '').trim() !== '');
  });

  readonly pageNumbers = computed<PaginationItem[]>(() => {
    const total = this.pageCount();
    const current = this.page();
    if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
    const pages: PaginationItem[] = [1];
    if (current > 4) pages.push('…');
    const start = Math.max(2, current - 2);
    const end = Math.min(total - 1, current + 2);
    for (let p = start; p <= end; p++) pages.push(p);
    if (current < total - 3) pages.push('…');
    pages.push(total);
    return pages;
  });

  get packet(): PacketDetails {
    return this.packetForm.getRawValue() as PacketDetails;
  }

  get environmentLabel(): string {
    return this.appConfig.value.application.defaultCaseDesignation;
  }

  get pageSizeOptions(): number[] {
    return this.appConfig.value.pagination.pageSizeOptions;
  }

  ngOnInit(): void {
    this.onlineWatcher();
    if (this.nativeMode) this.setupCsvImportEvents();
    this.packetForm.valueChanges.subscribe(() => {
      if (!this.restoringDraft) this.markDirty();
    });
    void this.restoreLocalDraft();
    if (this.nativeMode && this.tauriWindow) {
      void this.tauriWindow.onCloseRequested(async (event) => {
        event.preventDefault();
        await this.handleCloseRequested();
      }).then(unlisten => {
        this.closeUnlisten = unlisten;
      });
    } else {
      window.addEventListener('beforeunload', this.handleBrowserBeforeUnload);
    }
  }

  ngOnDestroy(): void {
    this.closeUnlisten?.();
    for (const unlisten of this.csvImportUnlisteners) unlisten();
    if (!this.nativeMode) window.removeEventListener('beforeunload', this.handleBrowserBeforeUnload);
  }

  private setupCsvImportEvents(): void {
    void Promise.all([
      listen<CsvImportProgress>('csv-import-progress', event => {
        this.ngZone.run(() => {
          const payload = event.payload;
          console.log('[TS] csv-import-progress received:', payload, 'current nativeRowStoreId:', this.nativeRowStoreId());
          if (payload.datasetId !== this.nativeRowStoreId()) {
            console.warn('[TS] datasetId mismatch! Payload:', payload.datasetId, 'Store:', this.nativeRowStoreId());
            return;
          }

          const wasReady = this.csvImportReady();
          console.log('[TS] wasReady:', wasReady, 'payload.ready:', payload.ready);
          this.csvImportActive.set(!payload.completed);
          this.csvImportReady.set(payload.ready);
          this.nativeTotalRows.set(payload.importedCount);
          this.nativeFilteredTotalRows.set(payload.importedCount);
          this.csvImportElapsedMs.set(payload.elapsedMs);
          this.csvImportProgress.set(
            payload.completed
              ? `${payload.importedCount.toLocaleString()} records loaded.`
              : `${payload.importedCount.toLocaleString()} records available — remaining records are loading in the background…`
          );

          // First ready event: close the modal immediately and show initial rows
          if (payload.ready && !wasReady) {
            console.log('[TS] Ready event! Closing modal and refreshing rows.');
            this.modal.set('none');
            this.importBusy.set(false);
            void this.refreshNativeRows();
          }

          if (payload.completed) {
            console.log('[TS] Import completed! Total rows:', payload.importedCount);
            this.csvImportActive.set(false);
            this.importBusy.set(false);
            this.nativeStoreDirty.set(true);
            this.markDirty();
            void this.refreshNativeRows();
          }
        });
      }),
      listen<CsvImportError>('csv-import-error', event => {
        this.ngZone.run(() => {
          const payload = event.payload;
          console.error('[TS] csv-import-error received:', payload);
          if (payload.datasetId !== this.nativeRowStoreId()) return;

          this.csvImportActive.set(false);
          this.csvImportReady.set(false);
          this.importBusy.set(false);
          this.csvImportProgress.set('');
          this.toast.show(payload.message, 'error');
        });
      })
    ]).then(unlisteners => {
      this.csvImportUnlisteners = unlisteners;
    }).catch(error => {
      console.error('Unable to register CSV import event listeners', error);
    });
  }

  goHome(): void {
    this.resetApplication();
    this.step.set(0);
  }

  startNew(): void {
    this.resetApplication();
    this.step.set(1);
    this.packetEditMode.set(true);
  }

  async openExisting(): Promise<void> {
    this.openDraftBusy.set(true);
    try {
      const [drafts, location] = await Promise.all([this.drafts.list(), this.drafts.getStorageLocation()]);
      this.savedDrafts.set(drafts);
      this.storageLocation.set(location);
      this.modal.set('openFile');
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      this.toast.show(`Unable to read saved drafts: ${msg}`, 'error');
    } finally {
      this.openDraftBusy.set(false);
    }
  }

  editPacket(): void {
    this.packetEditMode.set(true);
    this.toast.show('Packet details are editable now.', 'info');
  }

  closeModal(): void {
    if (this.csvImportActive()) return;
    this.modal.set('none');
    this.currentRowIndex.set(null);
    this.csvFile.set(null);
    this.csvNativePath.set(null);
    this.csvImportProgress.set('');
    this.generalDocumentFiles.set([]);
    this.currentDocumentId.set(null);
  }

  nextFromPacket(): void {
    this.packetForm.markAllAsTouched();
    if (this.packetForm.invalid) {
      this.toast.show('Complete all mandatory packet and submitting-person fields.', 'error');
      return;
    }
    this.packetEditMode.set(false);
    this.step.set(2);
  }

  back(): void {
    if (this.step() === 1) {
      this.step.set(0);
      return;
    }
    if (this.step() === 2) {
      this.packetEditMode.set(false);
      this.step.set(1);
      return;
    }
    if (this.step() === 3) this.step.set(2);
  }

  nextToCreate(): void {
    if (!this.packetForm.valid) {
      this.step.set(1);
      this.packetEditMode.set(true);
      this.toast.show('Complete packet details before proceeding.', 'error');
      return;
    }
    if (this.visibleRowCount() === 0) {
      this.toast.show('Add at least one row or import a CSV before creating the packet.', 'error');
      return;
    }
    this.step.set(3);
  }

  openAddRow(): void {
    this.currentRowIndex.set(null);
    this.rowForm.reset({ verificationStatus: 'Pending' });
    this.personOpen.set(true);
    this.infoOpen.set(true);
    this.verificationOpen.set(true);
    this.modal.set('addRow');
  }

  async editRow(index: number): Promise<void> {
    let row = this.rows()[index];
    if (this.nativeMode && row?.caseId && this.nativeRowStoreId()) {
      row = await invoke<PersonRow | null>('get_row_by_case_id', { datasetId: this.nativeRowStoreId(), caseId: row.caseId }) ?? row;
    }
    if (!row) return;
    if (row.verificationStatus === 'Completed') {
      this.toast.show('Completed rows are locked and cannot be edited.', 'info');
      return;
    }
    this.currentRowIndex.set(index);
    this.rowForm.patchValue({
      pan: row.pan, name: row.name, dobDoi: row.dobDoi, mobile: row.mobile, email: row.email, pinCode: row.pinCode, address: row.address,
      state: row.state, verificationStatus: row.verificationStatus, informationFy: row.informationDetails.informationFy,
      informationSourceType: row.informationDetails.informationSourceType, informationSourceDescription: row.informationDetails.informationSourceDescription,
      informationType: row.informationDetails.informationType, informationDescription: row.informationDetails.informationDescription,
      informationValue: row.informationDetails.informationValue, source: row.informationDetails.source, finding: row.informationDetails.finding,
      actionableAy: row.verificationDetails.actionableAy, statutoryReason: row.verificationDetails.statutoryReason,
      verificationResultType: row.verificationDetails.verificationResultType, incomeEscapingAssessmentValue: row.verificationDetails.incomeEscapingAssessmentValue,
      verificationInformationValue: row.verificationDetails.informationValue, resultDescription: row.verificationDetails.resultDescription
    });
    this.personOpen.set(true); this.infoOpen.set(true); this.verificationOpen.set(true); this.modal.set('addRow');
  }

  async saveRow(): Promise<void> {
    this.rowForm.markAllAsTouched();
    const invalidRequired = ['pan', 'name', 'dobDoi', 'mobile', 'email', 'pinCode', 'address', 'state', 'informationFy', 'informationType', 'finding', 'source', 'informationValue', 'actionableAy', 'statutoryReason', 'verificationResultType', 'incomeEscapingAssessmentValue', 'verificationInformationValue']
      .some(name => this.rowForm.get(name)?.invalid);
    if (invalidRequired) {
      this.toast.show('Complete all mandatory Person, Information and Verification details before saving.', 'error');
      if (this.hasPersonValidationError()) this.personOpen.set(true); else if (this.hasInformationValidationError()) this.infoOpen.set(true); else this.verificationOpen.set(true);
      return;
    }
    const existingIndex = this.currentRowIndex(); const existing = existingIndex !== null ? this.rows()[existingIndex] : null; const v = this.rowForm.getRawValue();
    const informationDetails: InformationDetails = { informationFy: v.informationFy!.trim(), informationSourceType: v.informationSourceType?.trim() || '', informationSourceDescription: v.informationSourceDescription?.trim() || '', informationType: v.informationType!.trim(), informationDescription: v.informationDescription?.trim() || '', informationValue: v.informationValue!.trim(), source: v.source!.trim(), finding: v.finding!.trim() };
    const verificationDetails: VerificationDetails = { actionableAy: v.actionableAy!.trim(), statutoryReason: v.statutoryReason!.trim(), verificationResultType: v.verificationResultType!.trim(), incomeEscapingAssessmentValue: v.incomeEscapingAssessmentValue!.trim(), informationValue: v.verificationInformationValue!.trim(), resultDescription: v.resultDescription?.trim() || '' };
    const nextIndex = this.nativeMode ? this.nativeTotalRows() + 1 : this.rows().length + 1;
    const row: PersonRow = { serialNo: existing?.serialNo ?? String(nextIndex).padStart(5, '0'), caseId: existing?.caseId ?? this.makeCaseId(nextIndex), pan: v.pan!.trim().toUpperCase(), name: v.name!.trim(), dobDoi: v.dobDoi!.trim(), mobile: v.mobile!.trim(), email: v.email!.trim().toLowerCase(), pinCode: v.pinCode!.trim(), address: v.address!.trim(), state: v.state!, verificationStatus: existing?.verificationStatus ?? 'Pending', informationDetails, verificationDetails };
    try {
      if (this.nativeMode) {
        await this.ensureNativeWritableStore(); const datasetId = this.nativeRowStoreId(); if (!datasetId) throw new Error('Unable to determine the local row store.');
        await invoke('upsert_row', { datasetId, row: this.toNativeRowInput(row) });
        if (existingIndex === null) this.nativeTotalRows.update(value => value + 1);
        this.markDirty(); await this.refreshNativeRows();
      } else {
        this.rows.update(current => { const next = [...current]; if (existingIndex === null) next.push(row); else next[existingIndex] = row; return next; }); this.markDirty(); this.page.set(this.pageCount());
      }
      this.closeModal(); this.toast.show(existingIndex === null ? 'Row saved successfully.' : 'Row updated successfully.', 'success');
    } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to save the row.', 'error'); }
  }

  private hasPersonValidationError(): boolean {
    return ['pan', 'name', 'dobDoi', 'mobile', 'email', 'pinCode', 'address', 'state'].some(name => this.rowForm.get(name)?.invalid);
  }

  private hasInformationValidationError(): boolean {
    return ['informationFy', 'informationType', 'finding', 'source', 'informationValue'].some(name => this.rowForm.get(name)?.invalid);
  }

  async deleteSelected(): Promise<void> {
    const selected = this.selectedRows(); if (!selected.size) { this.toast.show('Select at least one row to delete.', 'info'); return; } if (this.hasCompletedSelection()) { this.toast.show('Completed rows cannot be deleted.', 'info'); return; }
    try {
      if (this.nativeMode) {
        await this.ensureNativeWritableStore();
        await invoke('delete_rows', { datasetId: this.nativeRowStoreId(), caseIds: [...selected] });
        this.nativeTotalRows.update(value => Math.max(0, value - selected.size));
        this.markDirty(); this.selectedRows.set(new Set()); await this.refreshNativeRows();
      }
      else { const next = this.rows().filter(row => !selected.has(row.caseId)).map((row, i) => ({ ...row, serialNo: String(i + 1).padStart(5, '0') })); this.rows.set(next); this.selectedRows.set(new Set()); this.page.set(Math.min(this.page(), this.pageCount())); this.markDirty(); }
      this.toast.show('Selected rows deleted.', 'success');
    } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to delete selected rows.', 'error'); }
  }

  async validateRows(): Promise<void> {
    const selectedIds = this.selectedRows(); if (!selectedIds.size) { this.toast.show('Select one or more Approved rows before validating.', 'info'); return; } if (this.hasCompletedSelection()) { this.toast.show('Completed rows cannot be validated.', 'info'); return; }
    const selected = this.rows().filter(row => selectedIds.has(row.caseId)); const notApproved = selected.filter(row => row.verificationStatus !== 'Approved'); if (notApproved.length) { this.toast.show('Only rows with Approved status can be validated. Pending and Completed rows were not changed.', 'error'); return; }
    try { if (this.nativeMode) { await this.ensureNativeWritableStore(); await invoke('set_rows_status', { datasetId: this.nativeRowStoreId(), caseIds: [...selectedIds], status: 'Completed' }); await this.refreshNativeRows(); } else { this.rows.update(current => current.map(row => selectedIds.has(row.caseId) ? { ...row, verificationStatus: 'Completed' } : row)); } this.markDirty(); this.selectedRows.set(new Set()); this.toast.show(`${selected.length} Approved row(s) converted to Completed.`, 'success'); } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to validate rows.', 'error'); }
  }

  async setRowVerificationStatus(row: PersonRow, value: string): Promise<void> {
    if (row.verificationStatus === 'Completed' || (value !== 'Pending' && value !== 'Approved')) return;
    try { if (this.nativeMode) { await this.ensureNativeWritableStore(); await invoke('set_rows_status', { datasetId: this.nativeRowStoreId(), caseIds: [row.caseId], status: value }); await this.refreshNativeRows(); } else { this.rows.update(current => current.map(item => item.caseId === row.caseId ? { ...item, verificationStatus: value as VerificationStatus } : item)); } this.markDirty(); } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to update verification status.', 'error'); }
  }

  toggleRow(row: PersonRow, checked: boolean): void {
    this.selectedRows.update(set => {
      const next = new Set(set);
      if (checked) next.add(row.caseId); else next.delete(row.caseId);
      return next;
    });
  }

  allVisibleSelected(): boolean {
    const actionable = this.pagedRows().filter(row => row.verificationStatus !== 'Completed');
    return actionable.length > 0 && actionable.every(row => this.selectedRows().has(row.caseId));
  }

  toggleAllVisible(checked: boolean): void {
    this.selectedRows.update(set => {
      const next = new Set(set);
      for (const row of this.pagedRows()) {
        if (row.verificationStatus === 'Completed') {
          next.delete(row.caseId);
          continue;
        }
        if (checked) next.add(row.caseId); else next.delete(row.caseId);
      }
      return next;
    });
  }

  setFilter(value: string): void { this.filter.set(value); this.page.set(1); if (this.nativeMode) void this.refreshNativeRows(); }

  setPage(value: number | '…'): void { if (value === '…') return; this.page.set(Math.max(1, Math.min(this.pageCount(), value))); if (this.nativeMode) void this.refreshNativeRows(); }

  changePageSize(value: string): void { this.pageSize.set(Number(value)); this.page.set(1); if (this.nativeMode) void this.refreshNativeRows(); }

  openCsvModal(): void { this.clearCsvSelection(); this.modal.set('csv'); }

  clearCsvSelection(): void { this.csvFile.set(null); this.csvNativePath.set(null); this.csvImportProgress.set(''); this.importBusy.set(false); }

  async browseCsvFile(input?: HTMLInputElement): Promise<void> {
    if (this.importBusy()) return;
    if (!this.nativeMode) { input?.click(); return; }
    try { const picked = await invoke<NativePickedCsvFile | null>('pick_csv_file'); if (picked) { this.csvFile.set(new File([], picked.fileName, { type: 'text/csv' })); this.csvNativePath.set(picked.filePath); } } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to select the CSV file.', 'error'); }
  }

  handleCsvSelection(event: Event): void {
    if (this.importBusy()) return;
    const file = (event.target as HTMLInputElement).files?.[0] || null;
    this.csvFile.set(file); this.csvNativePath.set(null);
  }

  handleCsvDrop(event: DragEvent): void {
    event.preventDefault();
    if (this.importBusy()) return;
    const file = event.dataTransfer?.files?.[0] || null;
    if (file) { this.csvFile.set(file); this.csvNativePath.set(null); }
  }

  handleDocumentDrop(event: DragEvent): void {
    event.preventDefault();
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (!files.length) return;
    this.generalDocumentFiles.set(files.map(file => ({ file, fileSize: file.size })));
  }

  onDragOver(event: DragEvent): void {
    event.preventDefault();
  }

  async importCsv(): Promise<void> {
    const file = this.csvFile();
    let nativePath = this.csvNativePath();

    if (this.nativeMode && !nativePath) {
      try {
        const picked = await invoke<NativePickedCsvFile | null>('pick_csv_file');
        if (picked) {
          this.csvFile.set(new File([], picked.fileName, { type: 'text/csv' }));
          nativePath = picked.filePath;
          this.csvNativePath.set(nativePath);
        }
      } catch (error) {
        this.toast.show(
          error instanceof Error ? error.message : 'Unable to select the CSV file.',
          'error'
        );
        return;
      }
    }

    const selectedFile = this.csvFile();
    if (!selectedFile && !nativePath) {
      this.toast.show('Choose a CSV file first.', 'error');
      return;
    }

    this.importBusy.set(true);
    this.csvImportActive.set(true);
    this.csvImportReady.set(false);
    this.csvImportElapsedMs.set(null);
    this.csvImportProgress.set('Starting native CSV import…');

    try {
      if (this.nativeMode && nativePath) {
        // Important: this command starts the Rust worker and returns immediately.
        // The heavy CSV parser/SQLite writer runs outside the Tauri main thread.
        const datasetId = `rows-${crypto.randomUUID()}`;
        this.nativeRowStoreId.set(datasetId);
        this.nativeStoreDirty.set(true);
        this.page.set(1);
        this.filter.set('');
        this.rows.set([]);
        this.nativeTotalRows.set(0);
        this.nativeFilteredTotalRows.set(0);

        await invoke('import_csv_to_store', {
          datasetId,
          filePath: nativePath,
          referenceNumber: this.packet.referenceNumber
        });

        this.csvImportProgress.set(
          'Import worker started. Loading the first 100 records…'
        );
        this.markDirty();

        // Do not depend on the progress event as the only readiness signal. Polling the
        // native metadata gives us a deterministic first-page handoff even if an event is
        // delayed by the desktop runtime. The polling is asynchronous and never blocks the UI.
        await this.waitForInitialCsvRows(datasetId);
        return;
      }

      if (!selectedFile) {
        throw new Error('Choose a CSV file first.');
      }

      const maxBrowserBytes = 25 * 1024 * 1024;
      if (selectedFile.size > maxBrowserBytes) {
        throw new Error(
          'Large CSV files require the desktop Tauri import path.'
        );
      }

      const textContent = await selectedFile.text();
      const imported: PersonRow[] = [];
      const baseIndex = this.rows().length;
      let headers: string[] | null = null;
      let recordsRead = 0;

      for await (const record of this.streamCsvRecords(textContent)) {
        if (!headers) {
          headers = record.values.map(value =>
            value.trim().replace(/^\uFEFF/, '')
          );
          this.validateCsvHeaders(headers);
          continue;
        }

        if (record.values.length !== headers.length) {
          throw new Error(
            `Invalid CSV data at row ${record.rowNumber}: ` +
            `expected ${headers.length} columns but found ${record.values.length}.`
          );
        }

        const mapped = Object.fromEntries(
          headers.map((header, index) => [
            header,
            record.values[index] ?? ''
          ])
        ) as Record<string, string>;

        imported.push(
          this.mapCsvRecord(
            mapped,
            baseIndex + imported.length + 1,
            record.rowNumber
          )
        );

        recordsRead++;

        if (recordsRead % 2000 === 0) {
          this.csvImportProgress.set(
            `Validating ${recordsRead.toLocaleString()} records…`
          );
          await this.yieldToBrowser();
        }
      }

      if (!headers) {
        throw new Error('CSV file is empty.');
      }

      if (!imported.length) {
        throw new Error('CSV contains no data rows.');
      }

      this.rows.update(current => [
        ...current,
        ...imported
      ]);

      this.page.set(1);
      this.filter.set('');
      this.markDirty();

      this.modal.set('none');
      this.csvImportActive.set(false);
      this.importBusy.set(false);
      this.csvImportProgress.set('');
      this.toast.show(
        `${this.rows().length.toLocaleString()} records available successfully.`,
        'success'
      );
    } catch (error) {
      this.csvImportActive.set(false);
      this.csvImportReady.set(false);
      this.csvImportProgress.set('');

      const message = this.describeNativeError(
        error,
        'CSV import failed.'
      );

      this.importBusy.set(false);
      this.toast.show(
        message || 'CSV import failed.',
        'error'
      );
    }
  }

  private async waitForInitialCsvRows(datasetId: string): Promise<void> {
    const readyRows = Math.max(1, this.appConfig.value.csvImport.readyRows);
    const deadline = Date.now() + 15000;

    while (Date.now() < deadline && !this.csvImportReady()) {
      try {
        const result = await invoke<{ rows: PersonRow[]; totalCount: number }>('get_row_page', {
          datasetId,
          page: 1,
          pageSize: Math.min(readyRows, this.pageSize()),
          filter: ''
        });

        this.ngZone.run(() => {
          this.nativeTotalRows.set(result.totalCount);
          this.nativeFilteredTotalRows.set(result.totalCount);
          if (result.rows.length > 0) {
            this.rows.set(result.rows);
          }
        });

        if (result.totalCount >= readyRows || result.rows.length >= Math.min(readyRows, this.pageSize())) {
          this.ngZone.run(() => {
            this.csvImportReady.set(true);
            this.modal.set('none');
            this.importBusy.set(false);
            this.csvImportProgress.set(
              `${result.totalCount.toLocaleString()} records available — remaining records are loading in the background…`
            );
          });
          return;
        }
      } catch {
        // The import worker may not have committed its first batch yet. Keep polling.
      }

      await new Promise<void>(resolve => window.setTimeout(resolve, 50));
    }
  }

  async exportCsv(): Promise<void> {
    try {
      if (this.nativeMode) { const path = await invoke<string | null>('export_csv_file', { datasetId: this.nativeRowStoreId(), suggestedName: `${this.packet.referenceNumber || 'data-upload'}-rows.csv` }); if (path) this.toast.show(`CSV saved to ${path}.`, 'success'); return; }
      const esc = (value: string) => `"${value.replaceAll('"', '""')}"`; const lines = [this.csvHeaders.map(esc).join(',')]; for (const r of this.rows()) { lines.push([r.pan, r.name, r.dobDoi, r.mobile, r.email, r.pinCode, r.address, r.state, r.informationDetails.informationFy, r.informationDetails.informationType, r.informationDetails.finding, r.informationDetails.source, r.informationDetails.informationValue, r.informationDetails.informationDescription, r.verificationDetails.actionableAy, r.verificationDetails.verificationResultType, r.verificationDetails.statutoryReason, r.verificationDetails.incomeEscapingAssessmentValue, r.verificationDetails.informationValue].map(esc).join(',')); } await this.saveExport(lines.join('\r\n'), `${this.packet.referenceNumber || 'data-upload'}-rows.csv`);
    } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to export CSV.', 'error'); }
  }


  private async saveExport(content: string, suggestedName: string, mimeType = 'text/csv;charset=utf-8'): Promise<void> {
    const isCSV = mimeType.startsWith('text/csv');
    // Prepend UTF-8 BOM for CSV so Excel auto-detects encoding correctly
    const bom = isCSV ? '\uFEFF' : '';
    const blob = new Blob([bom + content], { type: mimeType });

    try {
      // 1️⃣ File System Access API — lets the user pick save location + enforces extension
      if ('showSaveFilePicker' in window) {
        try {
          const fileHandle = await (window as any).showSaveFilePicker({
            suggestedName,
            types: [{
              description: isCSV ? 'CSV Spreadsheet' : 'JSON File',
              accept: isCSV ? { 'text/csv': ['.csv'] } : { 'application/json': ['.json'] }
            }]
          });
          const writable = await fileHandle.createWritable();
          await writable.write(blob);
          await writable.close();
          this.toast.show(`${suggestedName} saved successfully.`, 'success');
          return;
        } catch (fsErr: any) {
          // AbortError means the user dismissed the picker — don't show an error toast
          if (fsErr?.name === 'AbortError') return;
          console.warn('showSaveFilePicker failed; trying next method.', fsErr);
        }
      }

      // 2️⃣ Tauri native save dialog (non-CSV only — CSV extension can't be enforced there)
      if (!isCSV && '__TAURI_INTERNALS__' in window) {
        try {
          const path = await invoke<string | null>('save_export_file', { suggestedName, content });
          if (path) {
            this.toast.show(`${suggestedName} saved to ${path}.`, 'success');
            return;
          }
          // null → user cancelled
          return;
        } catch (nativeError) {
          console.warn('Tauri native save failed; falling back to blob download.', nativeError);
        }
      }

      // 3️⃣ Final fallback — auto-download to default Downloads folder
      this.downloadBlob(blob, suggestedName);
      this.toast.show(`${suggestedName} downloaded to your Downloads folder.`, 'success');
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : `Unable to save ${suggestedName}.`, 'error');
    }
  }

  openGeneralDocument(): void {
    if (this.selectedRows().size === 0) {
      this.toast.show('Select at least one row before opening General Document.', 'info');
      return;
    }
    this.generalDocumentFiles.set([]);
    this.currentDocumentId.set(null);
    this.attachedForAll.set(false);
    this.modal.set('generalDoc');
  }

  async handleDocumentSelection(event: Event): Promise<void> {
    const files = Array.from((event.target as HTMLInputElement).files ?? []);
    if (!files.length) return;

    if (!('__TAURI_INTERNALS__' in window)) {
      this.generalDocumentFiles.set(files.map(file => ({
        file,
        fileSize: file.size
      })));
      return;
    }

    // The native picker provides stable filesystem references. We intentionally do not read all
    // selected files into memory; files are read one-at-a-time only when the packet is submitted.
    this.generalDocumentFiles.set(files.map(file => ({ file, fileSize: file.size })));
  }

  async browseForDocuments(input: HTMLInputElement): Promise<void> {
    if (!('__TAURI_INTERNALS__' in window)) {
      input.click();
      return;
    }

    try {
      const selection = await invoke<NativePickedDocument[]>('pick_supporting_documents');
      this.generalDocumentFiles.set(selection.map(item => ({
        file: new File([], item.file_name, { type: item.extension ? 'application/octet-stream' : 'application/octet-stream' }),
        filePath: item.file_path,
        fileSize: item.file_size
      })));
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to select the document(s).', 'error');
    }
  }

  saveGeneralDocument(): void {
    const selectedFiles = this.generalDocumentFiles();
    if (!selectedFiles.length) {
      this.toast.show('Choose at least one document first.', 'error');
      return;
    }
    if (!this.documentType().trim() || !this.documentDescription().trim()) {
      this.toast.show('Document Type and Description are required.', 'error');
      return;
    }

    for (const selected of selectedFiles) {
      if (selected.fileSize > 25 * 1024 * 1024) {
        this.toast.show(`Document '${selected.file.name}' is larger than 25 MB.`, 'error');
        return;
      }
    }

    const selectedRowIds = [...this.selectedRows()];
    const useAll = this.attachedForAll() && this.visibleRowCount() > 0;
    const targets = useAll ? this.rows().map(r => r.caseId) : selectedRowIds;
    if (!targets.length) {
      this.toast.show('Select at least one row for the document.', 'error');
      return;
    }

    const existingId = this.currentDocumentId();
    if (existingId) {
      const replacement = selectedFiles[0];
      const existing = this.documents().find(item => item.id === existingId);
      const doc: AttachedDocument = {
        id: existingId,
        fileName: replacement.file.name,
        docType: this.documentType().trim(),
        description: this.documentDescription().trim(),
        attachedFor: useAll ? 'All Rows' : 'Selected Rows',
        rowCaseIds: targets,
        file: replacement.file,
        filePath: replacement.filePath,
        fileSize: replacement.fileSize,
        fileType: replacement.file.type,
        lastModified: replacement.file.lastModified
      };
      this.documents.update(list => list.map(item => item.id === existingId ? doc : item));
    } else {
      const newDocuments: AttachedDocument[] = selectedFiles.map(selected => ({
        id: crypto.randomUUID(),
        fileName: selected.file.name,
        docType: this.documentType().trim(),
        description: this.documentDescription().trim(),
        attachedFor: useAll ? 'All Rows' : 'Selected Rows',
        rowCaseIds: targets,
        file: selected.file,
        filePath: selected.filePath,
        fileSize: selected.fileSize,
        fileType: selected.file.type,
        lastModified: selected.file.lastModified
      }));
      this.documents.update(list => [...list, ...newDocuments]);
    }

    this.currentDocumentId.set(null);
    this.generalDocumentFiles.set([]);
    this.modal.set('none');
    this.markDirty();
    this.toast.show(
      `${selectedFiles.length} document(s) attached to ${useAll ? 'all rows' : `${selectedRowIds.length} selected row(s)`}.`,
      'success'
    );
  }

  removeDocument(id: string): void { this.documents.update(list => list.filter(doc => doc.id !== id)); this.markDirty(); }

  editDocument(doc: AttachedDocument): void {
    this.currentDocumentId.set(doc.id);
    this.documentType.set(doc.docType);
    this.documentDescription.set(doc.description);
    this.generalDocumentFiles.set([{
      file: doc.file || new File([], doc.fileName, { type: doc.fileType || 'application/octet-stream', lastModified: doc.lastModified || Date.now() }),
      filePath: doc.filePath,
      fileSize: doc.fileSize ?? doc.file?.size ?? 0
    }]);
    this.attachedForAll.set(doc.attachedFor === 'All Rows');
    this.modal.set('generalDoc');
  }

  async saveDraft(): Promise<boolean> {
    const draftId = this.currentDraftId() ?? (this.nativeMode ? (this.nativeRowStoreId() ?? `draft-${crypto.randomUUID()}`) : undefined);
    if (this.nativeMode && !this.nativeRowStoreId()) this.nativeRowStoreId.set(draftId ?? null);
    const draft: DraftState = { version: 1, savedAt: new Date().toISOString(), step: this.step(), packet: this.packet, rows: this.nativeMode ? [] : this.rows(), rowStoreId: this.nativeMode ? this.nativeRowStoreId() ?? undefined : undefined, rowCount: this.nativeMode ? this.nativeTotalRows() : this.rows().length, documents: this.documents().map(doc => ({ id: doc.id, fileName: doc.fileName, docType: doc.docType, description: doc.description, attachedFor: doc.attachedFor, rowCaseIds: doc.rowCaseIds, filePath: doc.filePath, fileSize: doc.fileSize ?? doc.file?.size, fileType: doc.fileType ?? doc.file?.type, lastModified: doc.lastModified ?? doc.file?.lastModified })), draftId };
    try {
      const saved = await this.drafts.save(draft, this.documents());
      this.currentDraftId.set(saved.id);
      if (this.nativeMode) {
        this.nativeRowStoreId.set(saved.id);
        this.nativeTotalRows.set(saved.rowCount);
        await this.refreshNativeRows();
      }
      this.lastSavedAt.set(draft.savedAt);
      this.nativeStoreDirty.set(false);
      this.dirty.set(false);
      this.toast.show('Draft saved locally on this device.', 'success');
      return true;
    } catch (error) {
      const message = this.describeNativeError(error, 'Unable to save the draft locally.');
      this.toast.show(message || 'Unable to save the draft locally.', 'error');
      return false;
    }
  }

  async openSavedDraft(id: string): Promise<void> {
    this.openDraftBusy.set(true);
    try {
      const bundle = await this.drafts.loadById(id);
      if (!bundle) throw new Error('Saved draft could not be found.');
      await this.applyDraft(bundle);
      this.modal.set('none');
      this.toast.show('Existing draft opened.', 'success');
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to open saved draft.', 'error');
    } finally {
      this.openDraftBusy.set(false);
    }
  }

  async deleteSavedDraft(id: string): Promise<void> {
    try {
      await this.drafts.remove(id);
      this.savedDrafts.update(list => list.filter(item => item.id !== id));
      if (this.currentDraftId() === id) {
        this.currentDraftId.set(null);
        this.nativeRowStoreId.set(null);
        this.nativeTotalRows.set(0);
        this.nativeFilteredTotalRows.set(0);
        this.nativeStoreDirty.set(false);
        this.dirty.set(false);
      }
      this.toast.show('Saved draft deleted.', 'success');
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to delete saved draft.', 'error');
    }
  }

  handleDraftFile(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0]; if (!file) return;
    void file.text().then(text => { try { const draft = JSON.parse(text) as DraftState; if (draft.version !== 1 || !draft.packet || typeof draft.packet.referenceNumber !== 'string' || !Array.isArray(draft.rows)) throw new Error('Unsupported draft file. Export the draft using this utility and try again.'); if (!Array.isArray(draft.documents)) draft.documents = []; const invalidRow = draft.rows.find(row => !row.caseId || !row.pan || !row.name || !row.informationDetails || !row.verificationDetails); if (invalidRow) throw new Error('Draft contains an invalid person row.'); draft.draftId = undefined; void this.applyImportedDraft(draft); } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to open the draft file.', 'error'); } });
  }

  async createPacket(): Promise<void> {
    if (this.finalSubmitBusy()) return; this.finalSubmitBusy.set(true); this.progressMessage.set('Submitting packet metadata…');
    try {
      await this.api.submitPacket(this.packet); const documentIndex = this.buildDocumentIndex();
      if (this.nativeMode) { const total = this.nativeTotalRows(); this.progressMessage.set(`Submitting ${total.toLocaleString()} case record(s)…`); for (let startRow = 0; startRow < total; startRow += 100) { const pageNumber = Math.floor(startRow / 100) + 1; const data = await invoke<{ rows: PersonRow[]; totalCount: number }>('get_row_page', { datasetId: this.nativeRowStoreId(), page: pageNumber, pageSize: 100, filter: '' }); await this.runWithConcurrency(data.rows, 4, async row => { await this.api.submitCase(row, this.packet); for (const doc of documentIndex.get(row.caseId) ?? []) { this.progressMessage.set(`Uploading ${doc.fileName} for ${row.name}…`); const file = await this.materializeDocumentFile(doc); await this.api.uploadFile(file, row.caseId, { label: doc.fileName, type: doc.docType, description: doc.description, remarks: '' }); } }); } }
      else { this.progressMessage.set(`Submitting ${this.rows().length.toLocaleString()} case record(s)…`); await this.runWithConcurrency(this.rows(), 4, async row => { await this.api.submitCase(row, this.packet); for (const doc of documentIndex.get(row.caseId) ?? []) { this.progressMessage.set(`Uploading ${doc.fileName} for ${row.name}…`); const file = await this.materializeDocumentFile(doc); await this.api.uploadFile(file, row.caseId, { label: doc.fileName, type: doc.docType, description: doc.description, remarks: '' }); } }); }
      await this.drafts.clear(); if (this.nativeMode && this.nativeRowStoreId()) await invoke('clear_row_store', { datasetId: this.nativeRowStoreId() }); this.currentDraftId.set(null); this.nativeRowStoreId.set(null); this.nativeTotalRows.set(0); this.nativeFilteredTotalRows.set(0); this.nativeStoreDirty.set(false); this.dirty.set(false); this.progressMessage.set('Packet created successfully.'); this.modal.set('saveSuccess'); this.toast.show('Packet submitted successfully.', 'success');
    } catch (error) { this.toast.show(this.api.describeError(error), 'error'); this.progressMessage.set('Submission stopped. You can save the draft and retry later.'); } finally { this.finalSubmitBusy.set(false); }
  }

  private buildDocumentIndex(): Map<string, AttachedDocument[]> {
    const index = new Map<string, AttachedDocument[]>();
    for (const doc of this.documents()) {
      for (const caseId of doc.rowCaseIds) {
        const bucket = index.get(caseId);
        if (bucket) {
          bucket.push(doc);
        } else {
          index.set(caseId, [doc]);
        }
      }
    }
    return index;
  }

  private async materializeDocumentFile(doc: AttachedDocument): Promise<File> {
    if (doc.file && doc.file.size > 0) return doc.file;
    if (doc.filePath && '__TAURI_INTERNALS__' in window) {
      const bytes = await invoke<number[]>('read_file_bytes', { filePath: doc.filePath });
      return new File(
        [new Uint8Array(bytes)],
        doc.fileName,
        {
          type: doc.fileType || 'application/octet-stream',
          lastModified: doc.lastModified || Date.now()
        }
      );
    }
    throw new Error(`Document '${doc.fileName}' is not available. Re-select the document before submitting.`);
  }

  private async yieldToBrowser(): Promise<void> {
    await new Promise<void>(resolve => setTimeout(resolve, 0));
  }

  private async *streamCsvRecords(text: string): AsyncGenerator<{ values: string[]; rowNumber: number }> {
    let row: string[] = [];
    let field = '';
    let quoted = false;
    let rowNumber = 1;

    const emit = () => {
      const values = row;
      row = [];
      return values;
    };

    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      const next = text[i + 1];

      if (quoted && ch === '"' && next === '"') {
        field += '"';
        i++;
        continue;
      }

      if (ch === '"') {
        quoted = !quoted;
        continue;
      }

      if (!quoted && ch === ',') {
        row.push(field);
        field = '';
        continue;
      }

      if (!quoted && (ch === '\n' || ch === '\r')) {
        if (ch === '\r' && next === '\n') i++;
        row.push(field);
        field = '';
        const values = emit();
        if (values.some(v => v.trim() !== '')) {
          yield { values, rowNumber };
        }
        rowNumber++;
        continue;
      }

      field += ch;
    }

    if (quoted) {
      throw new Error(`Invalid CSV format: an opening quote was not closed.`);
    }

    if (field.length || row.length) {
      row.push(field);
      const values = emit();
      if (values.some(v => v.trim() !== '')) {
        yield { values, rowNumber };
      }
    }
  }

  private async runWithConcurrency<T>(items: T[], limit: number, worker: (item: T) => Promise<void>): Promise<void> {
    let cursor = 0;
    const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
      while (true) {
        const index = cursor++;
        if (index >= items.length) return;
        await worker(items[index]);
      }
    });
    await Promise.all(workers);
  }

  private describeNativeError(error: unknown, fallback: string): string {
    if (error instanceof Error) return error.message || fallback;
    if (typeof error === 'string') return error || fallback;
    try {
      const serialized = JSON.stringify(error);
      return serialized && serialized !== '{}' ? serialized : fallback;
    } catch {
      return fallback;
    }
  }

  private validateCsvHeaders(headers: string[]): void {
    const normalizedActual = headers.map(this.normalHeader);
    const normalizedExpected = this.csvHeaders.map(this.normalHeader);
    if (normalizedActual.length !== normalizedExpected.length || normalizedActual.some((header, index) => header !== normalizedExpected[index])) {
      throw new Error(`Invalid CSV format. Use Export CSV to get the required column order.`);
    }
  }

  private mapCsvRecord(record: Record<string, string>, index: number, csvRowNumber: number): PersonRow {
    const v = (header: string): string => record[header]?.trim() ?? '';
    const pan = v('PAN');
    const name = v('Name');
    const dobDoi = v('DOB/DOI');
    const mobile = v('Mobile');
    const email = v('E-Mail');
    const pinCode = v('PIN Code');
    const address = v('Address');
    const state = v('State');
    const informationFy = v('FY');
    const informationType = v('Information Type');
    const finding = v('Findings');
    const source = v('Source');
    const informationValue = v('Information Value');
    const description = v('Description');
    const actionableAy = v('Actionable AY');
    const verificationResultType = v('Verification Result Type');
    const statutoryReason = v('Statutory Reason');
    const incomeEscapingAssessmentValue = v('Income Escaping Assessment Value');
    const verificationInformationValue = v('Verification Information Value');

    const required = [
      ['PAN', pan], ['Name', name], ['DOB/DOI', dobDoi], ['Mobile', mobile], ['E-Mail', email], ['PIN Code', pinCode],
      ['Address', address], ['State', state], ['FY', informationFy], ['Information Type', informationType], ['Findings', finding],
      ['Source', source], ['Information Value', informationValue], ['Actionable AY', actionableAy],
      ['Verification Result Type', verificationResultType], ['Statutory Reason', statutoryReason],
      ['Income Escaping Assessment Value', incomeEscapingAssessmentValue], ['Verification Information Value', verificationInformationValue]
    ];
    const missing = required.find(([, value]) => !value);
    if (missing) throw new Error(`Invalid CSV data at row ${csvRowNumber}: ${missing[0]} is required.`);
    if (!/^[A-Za-z]{5}[0-9]{4}[A-Za-z]$/.test(pan)) throw new Error(`Invalid PAN at CSV row ${csvRowNumber}.`);
    if (!/^\d{10}$/.test(mobile)) throw new Error(`Invalid Mobile at CSV row ${csvRowNumber}.`);
    if (!/^\d{6}$/.test(pinCode)) throw new Error(`Invalid PIN Code at CSV row ${csvRowNumber}.`);
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) throw new Error(`Invalid E-Mail at CSV row ${csvRowNumber}.`);

    return {
      serialNo: String(index).padStart(5, '0'),
      caseId: this.makeCaseId(index),
      pan: pan.toUpperCase(),
      name,
      dobDoi,
      mobile,
      email: email.toLowerCase(),
      pinCode,
      address,
      state,
      verificationStatus: 'Pending',
      informationDetails: {
        informationFy,
        informationSourceType: '',
        informationSourceDescription: '',
        informationType,
        informationDescription: description,
        informationValue,
        source,
        finding
      },
      verificationDetails: {
        actionableAy,
        statutoryReason,
        verificationResultType,
        incomeEscapingAssessmentValue,
        informationValue: verificationInformationValue,
        resultDescription: ''
      }
    };
  }

  trackByCaseId(_index: number, row: PersonRow): string { return row.caseId; }

  private normalHeader(value: string): string {
    return value.trim().toLowerCase().replace(/[\s_\-./]+/g, '');
  }

  private makeCaseId(index: number): string {
    const ref = this.packet.referenceNumber.trim() || 'LOCAL';
    return `${ref}-${String(index).padStart(5, '0')}`;
  }

  private async restoreLocalDraft(): Promise<void> {
    try {
      const summaries = await this.drafts.list();
      const latest = summaries[0];
      if (latest) this.lastSavedAt.set(latest.updatedAt);
    } catch {
      // Local storage can be unavailable in browser fallback mode; the UI remains usable.
    }
  }

  private async applyDraft(bundle: LoadedDraftBundle): Promise<void> {
    const draft = bundle.draft; this.restoringDraft = true;
    try {
      this.packetForm.patchValue(draft.packet); this.documents.set(draft.documents.map(doc => { const stored = bundle.files[doc.id]; const file = stored ? new File([new Uint8Array(stored.bytes)], stored.name, { type: stored.type, lastModified: stored.lastModified || Date.now() }) : undefined; return { ...doc, file, fileSize: doc.fileSize ?? stored?.bytes?.length, fileType: doc.fileType ?? stored?.type, lastModified: doc.lastModified ?? stored?.lastModified }; })); this.currentDraftId.set(draft.draftId ?? null); this.step.set(Math.min(3, Math.max(1, draft.step))); this.packetEditMode.set(false); this.selectedRows.set(new Set()); this.page.set(1); this.filter.set(''); this.lastSavedAt.set(draft.savedAt); this.dirty.set(false); this.nativeStoreDirty.set(false);
      if (this.nativeMode) { this.nativeRowStoreId.set(draft.rowStoreId ?? draft.draftId ?? null); this.nativeTotalRows.set(draft.rowCount ?? draft.rows.length); await this.refreshNativeRows(); } else { this.rows.set(draft.rows); }
    } finally { this.restoringDraft = false; }
  }

  private async applyImportedDraft(draft: DraftState): Promise<void> {
    try { if (this.nativeMode) { const datasetId = `rows-${crypto.randomUUID()}`; this.nativeRowStoreId.set(datasetId); await invoke('seed_row_store', { datasetId, rows: draft.rows.map(row => this.toNativeRowInput(row)) }); this.nativeTotalRows.set(draft.rows.length); draft.rowStoreId = datasetId; draft.rowCount = draft.rows.length; } await this.applyDraft({ draft, files: {} }); this.modal.set('none'); this.markDirty(); const localDocuments = draft.documents.filter(doc => !!doc.filePath).length; const missingDocuments = draft.documents.length - localDocuments; if (missingDocuments > 0) this.toast.show(`Draft imported with ${missingDocuments} document reference(s). Re-select any missing local files before final submission.`, 'info'); else this.toast.show('Existing draft file opened.', 'success'); } catch (error) { this.toast.show(error instanceof Error ? error.message : 'Unable to open the draft file.', 'error'); }
  }

  private resetApplication(): void { this.packetForm.reset(); this.rows.set([]); this.documents.set([]); this.selectedRows.set(new Set()); this.currentRowIndex.set(null); this.currentDraftId.set(null); this.nativeRowStoreId.set(null); this.nativeTotalRows.set(0); this.nativeFilteredTotalRows.set(0); this.nativeStoreDirty.set(false); this.dirty.set(false); this.page.set(1); this.filter.set(''); this.lastSavedAt.set(null); this.progressMessage.set(''); this.packetEditMode.set(true); this.modal.set('none'); }

  private markDirty(): void { if (this.restoringDraft) return; this.dirty.set(true); if (this.nativeMode) this.nativeStoreDirty.set(true); }

  private async ensureNativeWritableStore(): Promise<void> { if (!this.nativeMode) return; if (!this.nativeRowStoreId()) this.nativeRowStoreId.set(`rows-${crypto.randomUUID()}`); if (this.nativeStoreDirty() || !this.currentDraftId()) return; const current = this.nativeRowStoreId(); if (!current) return; const next = `rows-${crypto.randomUUID()}`; await invoke('clone_row_store', { sourceDatasetId: current, targetDatasetId: next }); this.nativeRowStoreId.set(next); this.nativeStoreDirty.set(true); }

  private async refreshNativeRows(): Promise<void> { if (!this.nativeMode || !this.nativeRowStoreId()) return; const result = await invoke<{ rows: PersonRow[]; totalCount: number }>('get_row_page', { datasetId: this.nativeRowStoreId(), page: this.page(), pageSize: this.pageSize(), filter: this.filter() }); this.rows.set(result.rows); this.nativeFilteredTotalRows.set(result.totalCount); const maxPage = Math.max(1, Math.ceil(result.totalCount / this.pageSize())); if (this.page() > maxPage) this.page.set(maxPage); }

  private toNativeRowInput(row: PersonRow): unknown { return { serialNo: row.serialNo, caseId: row.caseId, pan: row.pan, name: row.name, dobDoi: row.dobDoi, mobile: row.mobile, email: row.email, pinCode: row.pinCode, address: row.address, state: row.state, verificationStatus: row.verificationStatus, informationDetails: row.informationDetails, verificationDetails: row.verificationDetails }; }

  private async handleCloseRequested(): Promise<void> {
    if (this.closing()) return;
    if (this.finalSubmitBusy() || this.importBusy()) {
      this.toast.show('Please wait for the current operation to finish before closing the utility.', 'info');
      return;
    }
    if (!this.hasDataOnScreen()) {
      await this.exitUtility();
      return;
    }
    this.modal.set('closeConfirm');
  }

  async saveDraftAndExit(): Promise<void> {
    if (this.closing()) return;
    if (await this.saveDraft()) await this.exitUtility();
  }

  async exitWithoutSaving(): Promise<void> {
    if (this.closing()) return;

    // Explicit exit means the window must close immediately. Cleanup is best-effort and
    // intentionally fire-and-forget so a large local SQLite dataset can never hold the
    // close flow hostage. The native command itself runs on a background task.
    if (this.nativeMode && this.nativeRowStoreId() && this.nativeStoreDirty() && (!this.currentDraftId() || this.nativeRowStoreId() !== this.currentDraftId())) {
      const datasetId = this.nativeRowStoreId();
      void invoke('clear_row_store', { datasetId })
        .catch(error => console.warn('Background row-store cleanup failed.', error));
    }

    await this.exitUtility();
  }

  private async exitUtility(): Promise<void> {
    if (this.closing()) return;
    this.closing.set(true);
    if (this.tauriWindow) {
      try {
        await this.tauriWindow.destroy();
      } catch (error) {
        this.closing.set(false);
        this.toast.show(this.describeNativeError(error, 'Unable to close the utility.'), 'error');
      }
    }
  }
  increaseZoom(): void { this.setZoom(Math.min(125, this.zoomPercent() + 10)); }
  decreaseZoom(): void { this.setZoom(Math.max(80, this.zoomPercent() - 10)); }
  private setZoom(value: number): void { this.zoomPercent.set(value); document.documentElement.style.setProperty('--app-zoom', String(value / 100)); }
  skipToMain(): void { const main = document.getElementById('main-content'); if (main) { main.focus(); main.scrollIntoView({ behavior: 'smooth', block: 'start' }); } }
  async openGovernmentSite(): Promise<void> {
    if (!this.nativeMode) {
      window.open(this.appConfig.value.application.governmentWebsiteUrl, '_blank', 'noopener,noreferrer');
      return;
    }
    try {
      await invoke('open_government_website', { url: this.appConfig.value.application.governmentWebsiteUrl });
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to open the Government of India website.', 'error');
    }
  }
  private readonly handleBrowserBeforeUnload = (event: BeforeUnloadEvent): void => { if (!this.dirty() || !this.hasDataOnScreen()) return; event.preventDefault(); event.returnValue = ''; };

  private onlineWatcher(): void {
    const update = () => this.isOnline.set(navigator.onLine);
    update();
    window.addEventListener('online', update);
    window.addEventListener('offline', update);
    window.setInterval(async () => {
      if (!navigator.onLine) {
        this.isOnline.set(false);
        return;
      }
      this.isOnline.set(await this.api.health());
    }, 10000);
  }

  private downloadBlob(blob: Blob, name: string): void {
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
}
