import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { invoke } from '@tauri-apps/api/core';
import { ApiService } from './services/api.service';
import { DraftStoreService } from './services/draft-store.service';
import { ToastService } from './services/toast.service';
import { environment } from '../environments/environment';
import { AttachedDocument, DraftState, InformationDetails, PacketDetails, PersonRow, VerificationDetails } from './models';

@Component({
  selector: 'idu-root',
  standalone: true,
  imports: [CommonModule, ReactiveFormsModule],
  templateUrl: './app.component.html',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class AppComponent implements OnInit {
  private readonly fb = inject(FormBuilder);
  readonly api = inject(ApiService);
  readonly drafts = inject(DraftStoreService);
  readonly toast = inject(ToastService);

  readonly step = signal(0); // 0 home, 1 packet, 2 people, 3 create
  readonly modal = signal<'none' | 'addRow' | 'info' | 'verification' | 'csv' | 'generalDoc' | 'openFile' | 'saveSuccess'>('none');
  readonly infoOpen = signal(false);
  readonly verificationOpen = signal(false);
  readonly personOpen = signal(true);
  readonly infoModalOpen = signal(false);
  readonly verificationModalOpen = signal(false);
  readonly isOnline = signal(true);
  readonly busy = signal(false);
  readonly importBusy = signal(false);
  readonly finalSubmitBusy = signal(false);
  readonly currentRowIndex = signal<number | null>(null);
  readonly selectedRows = signal<Set<string>>(new Set());
  readonly pageSize = signal(10);
  readonly page = signal(1);
  readonly filter = signal('');
  readonly lastSavedAt = signal<string | null>(null);
  readonly progressMessage = signal('');
  readonly documentType = signal('Verification Type');
  readonly documentDescription = signal('Found this digitally');
  readonly attachedForAll = signal(true);

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
    verificationStatus: ['Pending' as 'Pending' | 'Completed', Validators.required],
    informationFy: [''],
    informationSourceType: [''],
    informationSourceDescription: [''],
    informationType: [''],
    informationDescription: [''],
    informationValue: [''],
    source: [''],
    finding: [''],
    actionableAy: [''],
    statutoryReason: [''],
    verificationResultType: [''],
    incomeEscapingAssessmentValue: [''],
    verificationInformationValue: [''],
    resultDescription: ['']
  });

  readonly rows = signal<PersonRow[]>([]);
  readonly documents = signal<AttachedDocument[]>([]);
  readonly csvFile = signal<File | null>(null);
  readonly generalDocumentFile = signal<File | null>(null);
  readonly currentDocumentId = signal<string | null>(null);

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

  ngOnInit(): void {
    this.onlineWatcher();
    this.restoreLocalDraft();
  }

  get packet(): PacketDetails {
    return this.packetForm.getRawValue() as PacketDetails;
  }

  get environmentLabel(): string {
    return environment.defaultCaseDesignation;
  }

  get filteredRows(): PersonRow[] {
    const term = this.filter().trim().toLowerCase();
    const source = this.rows();
    if (!term) return source;
    return source.filter(r => [r.pan, r.name, r.email, r.address, r.state, r.serialNo].some(v => v.toLowerCase().includes(term)));
  }

  get pagedRows(): PersonRow[] {
    const filtered = this.filteredRows;
    const start = (this.page() - 1) * this.pageSize();
    return filtered.slice(start, start + this.pageSize());
  }

  get pageCount(): number {
    return Math.max(1, Math.ceil(this.filteredRows.length / this.pageSize()));
  }

  goHome(): void {
    this.resetApplication();
    this.step.set(0);
  }

  startNew(): void {
    this.resetApplication();
    this.step.set(1);
  }

  async openExisting(): Promise<void> {
    this.modal.set('openFile');
  }

  closeModal(): void {
    this.modal.set('none');
    this.currentRowIndex.set(null);
    this.csvFile.set(null);
    this.generalDocumentFile.set(null);
    this.currentDocumentId.set(null);
  }

  nextFromPacket(): void {
    this.packetForm.markAllAsTouched();
    if (this.packetForm.invalid) {
      this.toast.show('Complete all mandatory packet and submitting-person fields.', 'error');
      return;
    }
    this.step.set(2);
  }

  back(): void {
    if (this.step() === 1) this.step.set(0);
    else if (this.step() === 2) this.step.set(1);
    else if (this.step() === 3) this.step.set(2);
  }

  nextToCreate(): void {
    if (!this.packetForm.valid) {
      this.step.set(1);
      this.toast.show('Complete packet details before proceeding.', 'error');
      return;
    }
    if (!this.rows().length) {
      this.toast.show('Add at least one row or import a CSV before creating the packet.', 'error');
      return;
    }
    this.step.set(3);
  }

  openAddRow(): void {
    this.rowForm.reset({ verificationStatus: 'Pending' });
    this.personOpen.set(true);
    this.infoOpen.set(false);
    this.verificationOpen.set(false);
    this.infoModalOpen.set(false);
    this.verificationModalOpen.set(false);
    this.modal.set('addRow');
  }

  editRow(index: number): void {
    const row = this.rows()[index];
    if (!row) return;
    this.currentRowIndex.set(index);
    this.rowForm.patchValue({
      pan: row.pan,
      name: row.name,
      dobDoi: row.dobDoi,
      mobile: row.mobile,
      email: row.email,
      pinCode: row.pinCode,
      address: row.address,
      state: row.state,
      verificationStatus: row.verificationStatus,
      informationFy: row.informationDetails.informationFy,
      informationSourceType: row.informationDetails.informationSourceType,
      informationSourceDescription: row.informationDetails.informationSourceDescription,
      informationType: row.informationDetails.informationType,
      informationDescription: row.informationDetails.informationDescription,
      informationValue: row.informationDetails.informationValue,
      source: row.informationDetails.source,
      finding: row.informationDetails.finding,
      actionableAy: row.verificationDetails.actionableAy,
      statutoryReason: row.verificationDetails.statutoryReason,
      verificationResultType: row.verificationDetails.verificationResultType,
      incomeEscapingAssessmentValue: row.verificationDetails.incomeEscapingAssessmentValue,
      verificationInformationValue: row.verificationDetails.informationValue,
      resultDescription: row.verificationDetails.resultDescription
    });
    this.personOpen.set(true);
    this.infoOpen.set(false);
    this.verificationOpen.set(false);
    this.modal.set('addRow');
  }

  saveRow(): void {
    this.rowForm.markAllAsTouched();
    const panControl = this.rowForm.controls.pan;
    if (panControl.errors?.['required']) {
      this.toast.show('PAN is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (panControl.errors?.['pattern']) {
      this.toast.show('Invalid PAN format: Must be 10 characters with 5 letters, 4 numbers, 1 letter (e.g. ABCDE1234F).', 'error');
      this.personOpen.set(true);
      return;
    }
    if (this.rowForm.controls.name.invalid) {
      this.toast.show('Name is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (this.rowForm.controls.dobDoi.invalid) {
      this.toast.show('DOB/DOI is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    const mobileControl = this.rowForm.controls.mobile;
    if (mobileControl.errors?.['required']) {
      this.toast.show('Mobile number is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (mobileControl.errors?.['pattern']) {
      this.toast.show('Invalid Mobile number: Must be exactly 10 digits.', 'error');
      this.personOpen.set(true);
      return;
    }
    const emailControl = this.rowForm.controls.email;
    if (emailControl.errors?.['required']) {
      this.toast.show('E-mail is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (emailControl.errors?.['email']) {
      this.toast.show('Invalid E-mail address format.', 'error');
      this.personOpen.set(true);
      return;
    }
    const pinControl = this.rowForm.controls.pinCode;
    if (pinControl.errors?.['required']) {
      this.toast.show('PIN Code is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (pinControl.errors?.['pattern']) {
      this.toast.show('Invalid PIN Code: Must be exactly 6 digits.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (this.rowForm.controls.address.invalid) {
      this.toast.show('Address is required.', 'error');
      this.personOpen.set(true);
      return;
    }
    if (this.rowForm.controls.state.invalid) {
      this.toast.show('State is required.', 'error');
      this.personOpen.set(true);
      return;
    }

    const v = this.rowForm.getRawValue();
    const informationDetails: InformationDetails = {
      informationFy: v.informationFy || '',
      informationSourceType: v.informationSourceType || '',
      informationSourceDescription: v.informationSourceDescription || '',
      informationType: v.informationType || '',
      informationDescription: v.informationDescription || '',
      informationValue: v.informationValue || '',
      source: v.source || '',
      finding: v.finding || ''
    };
    const verificationDetails: VerificationDetails = {
      actionableAy: v.actionableAy || '',
      statutoryReason: v.statutoryReason || '',
      verificationResultType: v.verificationResultType || '',
      incomeEscapingAssessmentValue: v.incomeEscapingAssessmentValue || '',
      informationValue: v.verificationInformationValue || '',
      resultDescription: v.resultDescription || ''
    };

    const existingIndex = this.currentRowIndex();
    const existing = existingIndex !== null ? this.rows()[existingIndex] : null;
    const row: PersonRow = {
      serialNo: existing?.serialNo ?? String(this.rows().length + 1).padStart(5, '0'),
      caseId: existing?.caseId ?? this.makeCaseId(this.rows().length + 1),
      pan: v.pan!.trim().toUpperCase(),
      name: v.name!.trim(),
      dobDoi: v.dobDoi!.trim(),
      mobile: v.mobile!.trim(),
      email: v.email!.trim().toLowerCase(),
      pinCode: v.pinCode!.trim(),
      address: v.address!.trim(),
      state: v.state!,
      verificationStatus: v.verificationStatus!,
      informationDetails,
      verificationDetails
    };

    this.rows.update(current => {
      const next = [...current];
      if (existingIndex === null) next.push(row); else next[existingIndex] = row;
      return next;
    });
    this.page.set(this.pageCount);
    this.closeModal();
    this.toast.show(existingIndex === null ? 'Row added successfully.' : 'Row updated successfully.', 'success');
  }

  deleteSelected(): void {
    const selected = this.selectedRows();
    if (!selected.size) {
      this.toast.show('Select at least one row to delete.', 'info');
      return;
    }
    const next = this.rows().filter(row => !selected.has(row.caseId)).map((row, i) => ({ ...row, serialNo: String(i + 1).padStart(5, '0') }));
    this.rows.set(next);
    this.selectedRows.set(new Set());
    this.toast.show('Selected rows deleted.', 'success');
  }

  validateRows(): void {
    if (!this.rows().length) {
      this.toast.show('There are no rows to validate.', 'info');
      return;
    }
    const invalid = this.rows().filter(r => !r.pan || !r.name || !r.mobile || !r.email || !r.pinCode || !r.address || !r.state);
    if (invalid.length) this.toast.show(`${invalid.length} row(s) have missing mandatory fields.`, 'error');
    else this.toast.show(`${this.rows().length} row(s) passed local validation.`, 'success');
  }

  toggleRow(row: PersonRow, checked: boolean): void {
    this.selectedRows.update(set => {
      const next = new Set(set);
      if (checked) next.add(row.caseId); else next.delete(row.caseId);
      return next;
    });
  }

  allVisibleSelected(): boolean {
    const visible = this.pagedRows;
    return visible.length > 0 && visible.every(row => this.selectedRows().has(row.caseId));
  }

  toggleAllVisible(checked: boolean): void {
    this.selectedRows.update(set => {
      const next = new Set(set);
      for (const row of this.pagedRows) {
        if (checked) next.add(row.caseId); else next.delete(row.caseId);
      }
      return next;
    });
  }

  setFilter(value: string): void {
    this.filter.set(value);
    this.page.set(1);
  }

  setPage(value: number): void {
    this.page.set(Math.max(1, Math.min(this.pageCount, value)));
  }

  changePageSize(value: string): void {
    this.pageSize.set(Number(value));
    this.page.set(1);
  }

  openCsvModal(): void {
    this.csvFile.set(null);
    this.modal.set('csv');
  }

  handleCsvSelection(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0] || null;
    this.csvFile.set(file);
  }

  handleCsvDrop(event: DragEvent): void {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0] || null;
    if (file) this.csvFile.set(file);
  }

  handleDocumentDrop(event: DragEvent): void {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0] || null;
    if (file) this.generalDocumentFile.set(file);
  }

  handleDraftDrop(event: DragEvent): void {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0] || null;
    if (file) {
      void file.text().then(text => {
        try {
          const draft = JSON.parse(text) as DraftState;
          if (draft.version !== 1 || !draft.packet || !Array.isArray(draft.rows)) throw new Error('Unsupported draft file.');
          this.applyDraft(draft);
          this.modal.set('none');
          this.toast.show('Existing draft opened.', 'success');
        } catch (error) {
          this.toast.show(error instanceof Error ? error.message : 'Unable to open the draft file.', 'error');
        }
      });
    }
  }

  onDragOver(event: DragEvent): void {
    event.preventDefault();
  }

  async importCsv(): Promise<void> {
    const file = this.csvFile();
    if (!file) {
      this.toast.show('Choose a CSV file first.', 'error');
      return;
    }
    if (file.size > 25 * 1024 * 1024) {
      this.toast.show('CSV files larger than 25 MB are not allowed.', 'error');
      return;
    }
    this.importBusy.set(true);
    try {
      const text = await file.text();
      const records = this.parseCsv(text);
      const imported: PersonRow[] = [];
      records.forEach((record, index) => {
        const row = this.mapCsvRecord(record, this.rows().length + index + 1);
        if (row) imported.push(row);
      });
      if (!imported.length) throw new Error('No usable records were found in the CSV.');
      this.rows.update(current => [...current, ...imported]);
      this.page.set(1);
      this.modal.set('none');
      this.toast.show(`${imported.length} records imported successfully.`, 'success');
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'CSV import failed.', 'error');
    } finally {
      this.importBusy.set(false);
    }
  }

  exportCsv(): void {
    const headers = ['Sr. No.', 'PAN', 'Name', 'DOB/DOI', 'Mobile', 'E-Mail', 'PIN Code', 'Address', 'State', 'Verification Status', 'Information FY', 'Information Source Type', 'Information Source Description', 'Information Type', 'Information Description', 'Information Value', 'Source', 'Finding', 'Actionable AY', 'Statutory Reason', 'Verification Result Type', 'Income Escaping Assessment Value', 'Verification Information Value', 'Verification Result Description'];
    const esc = (v: string) => `"${v.replaceAll('\"', '\"\"')}"`;
    const lines = [headers.map(esc).join(',')];
    for (const r of this.rows()) {
      lines.push([r.serialNo, r.pan, r.name, r.dobDoi, r.mobile, r.email, r.pinCode, r.address, r.state, r.verificationStatus, r.informationDetails.informationFy, r.informationDetails.informationSourceType, r.informationDetails.informationSourceDescription, r.informationDetails.informationType, r.informationDetails.informationDescription, r.informationDetails.informationValue, r.informationDetails.source, r.informationDetails.finding, r.verificationDetails.actionableAy, r.verificationDetails.statutoryReason, r.verificationDetails.verificationResultType, r.verificationDetails.incomeEscapingAssessmentValue, r.verificationDetails.informationValue, r.verificationDetails.resultDescription].map(esc).join(','));
    }
    this.downloadBlob(new Blob([lines.join('\n')], { type: 'text/csv;charset=utf-8' }), `${this.packet.referenceNumber || 'data-upload'}-rows.csv`);
  }

  downloadSampleCsv(): void {
    const csv = `Sr. No.,PAN,Name,DOB/DOI,Mobile,E-Mail,PIN Code,Address,State,Verification Status,Information FY,Information Source Type,Information Type,Information Value,Actionable AY,Verification Result Type\n00001,XZQWE9876J,Neha Sharma,15/03/1985,9876543210,neha.sharma@example.in,400050,"Bandra West, Mumbai",Maharashtra,Completed,2025-26,Digital,Financial Information,125000,2026-27,Income Escaping Assessment\n00002,LMNOP4321K,Ravi Kumar,08/11/1990,9876543211,ravi.kumar@example.in,560034,"Koramangala, Bangalore",Karnataka,Completed,2025-26,Portal,Banking Information,84000,2026-27,Further Verification Required\n00003,QWERT5678L,Sonal Mehta,29/07/1982,9876543212,sonal.mehta@example.in,700064,"Salt Lake, Kolkata",West Bengal,Pending,2024-25,Third Party,Property Information,2400000,2025-26,No Income Escaping\n`;
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
    this.downloadBlob(blob, 'sample-data-upload.csv');
  }

  openGeneralDocument(): void {
    if (!this.rows().length) {
      this.toast.show('Add or import at least one row before attaching a general document.', 'info');
      return;
    }
    this.generalDocumentFile.set(null);
    this.currentDocumentId.set(null);
    this.attachedForAll.set(this.selectedRows().size === 0);
    this.modal.set('generalDoc');
  }

  handleDocumentSelection(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0] || null;
    this.generalDocumentFile.set(file);
  }

  async browseForDocument(input: HTMLInputElement): Promise<void> {
    if (!('__TAURI_INTERNALS__' in window)) {
      input.click();
      return;
    }

    try {
      const selection = await invoke<[string, number[]] | null>('pick_file');
      if (!selection) return;

      const [name, bytes] = selection;
      this.generalDocumentFile.set(new File([new Uint8Array(bytes)], name));
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to select the document.', 'error');
    }
  }

  saveGeneralDocument(): void {
    const file = this.generalDocumentFile();
    if (!file) {
      this.toast.show('Choose a document first.', 'error');
      return;
    }
    if (file.size > 25 * 1024 * 1024) {
      this.toast.show('Documents larger than 25 MB are not allowed by the utility UI.', 'error');
      return;
    }
    const selected = [...this.selectedRows()];
    const useAll = this.attachedForAll() || selected.length === 0;
    const targets = useAll ? this.rows().map(r => r.caseId) : selected;
    const existingId = this.currentDocumentId();
    const doc: AttachedDocument = {
      id: existingId || crypto.randomUUID(),
      fileName: file.name,
      docType: this.documentType(),
      description: this.documentDescription(),
      attachedFor: useAll ? 'All Rows' : 'Selected Rows',
      rowCaseIds: targets,
      file
    };
    this.documents.update(list => existingId ? list.map(item => item.id === existingId ? doc : item) : [...list, doc]);
    this.currentDocumentId.set(null);
    this.modal.set('none');
    this.toast.show(`Document ${useAll ? 'attached to all rows' : `attached to ${selected.length} selected row(s)`}.`, 'success');
  }

  removeDocument(id: string): void {
    this.documents.update(list => list.filter(doc => doc.id !== id));
  }

  editDocument(doc: AttachedDocument): void {
    this.currentDocumentId.set(doc.id);
    this.documentType.set(doc.docType);
    this.documentDescription.set(doc.description);
    this.generalDocumentFile.set(doc.file || null);
    this.attachedForAll.set(doc.attachedFor === 'All Rows');
    this.modal.set('generalDoc');
  }

  async saveDraft(): Promise<void> {
    const draft: DraftState = {
      version: 1,
      savedAt: new Date().toISOString(),
      step: this.step(),
      packet: this.packet,
      rows: this.rows(),
      documents: this.documents().map(doc => ({
        id: doc.id,
        fileName: doc.fileName,
        docType: doc.docType,
        description: doc.description,
        attachedFor: doc.attachedFor,
        rowCaseIds: doc.rowCaseIds
      }))
    };
    try {
      await this.drafts.save(draft, this.documents());
      this.lastSavedAt.set(draft.savedAt);
      this.toast.show('Draft saved locally on this device.', 'success');
    } catch (error) {
      this.toast.show(error instanceof Error ? error.message : 'Unable to save the draft locally.', 'error');
    }
  }

  async loadDraftFromBrowser(): Promise<void> {
    const bundle = await this.drafts.load();
    if (!bundle) {
      this.toast.show('No local draft was found.', 'info');
      return;
    }
    this.applyDraft(bundle.draft, bundle.files);
    this.toast.show('Local draft restored.', 'success');
    this.modal.set('none');
  }

  handleDraftFile(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (!file) return;
    void file.text().then(text => {
      try {
        const draft = JSON.parse(text) as DraftState;
        if (draft.version !== 1 || !draft.packet || !Array.isArray(draft.rows)) throw new Error('Unsupported draft file.');
        this.applyDraft(draft);
        this.modal.set('none');
        this.toast.show('Existing draft opened.', 'success');
      } catch (error) {
        this.toast.show(error instanceof Error ? error.message : 'Unable to open the draft file.', 'error');
      }
    });
  }

  exportDraftFile(): void {
    const draft: DraftState = {
      version: 1,
      savedAt: new Date().toISOString(),
      step: this.step(),
      packet: this.packet,
      rows: this.rows(),
      documents: this.documents().map(doc => ({
        id: doc.id,
        fileName: doc.fileName,
        docType: doc.docType,
        description: doc.description,
        attachedFor: doc.attachedFor,
        rowCaseIds: doc.rowCaseIds
      }))
    };
    const blob = new Blob([JSON.stringify(draft, null, 2)], { type: 'application/json' });
    const ref = this.packet.referenceNumber.trim() || 'data-upload-draft';
    this.downloadBlob(blob, `${ref}-draft.json`);
  }

  async createPacket(): Promise<void> {
    if (this.finalSubmitBusy()) return;
    this.finalSubmitBusy.set(true);
    this.progressMessage.set('Submitting packet metadata…');
    try {
      await this.api.submitPacket(this.packet);
      this.progressMessage.set(`Submitting ${this.rows().length} case record(s)…`);
      await this.runWithConcurrency(this.rows(), 4, async (row) => {
        await this.api.submitCase(row, this.packet);
        const docs = this.documents().filter(d => d.rowCaseIds.includes(row.caseId) && d.file);
        for (const doc of docs) {
          this.progressMessage.set(`Uploading ${doc.fileName} for ${row.name}…`);
          await this.api.uploadFile(doc.file!, row.caseId, {
            label: doc.fileName,
            type: doc.docType,
            description: doc.description,
            remarks: ''
          });
        }
      });
      await this.drafts.clear();
      this.progressMessage.set('Packet created successfully.');
      this.modal.set('saveSuccess');
      this.toast.show('Packet submitted successfully.', 'success');
    } catch (error) {
      this.toast.show(this.api.describeError(error), 'error');
      this.progressMessage.set('Submission stopped. You can save the draft and retry later.');
    } finally {
      this.finalSubmitBusy.set(false);
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

  private mapCsvRecord(record: Record<string, string>, index: number): PersonRow | null {
    const v = (keys: string[]): string => {
      for (const key of keys) {
        const found = Object.entries(record).find(([header]) => this.normalHeader(header) === this.normalHeader(key));
        if (found?.[1]) return found[1].trim();
      }
      return '';
    };

    const pan = v(['PAN', 'Source PAN']);
    const name = v(['Name']);
    if (!pan || !name) return null;
    return {
      serialNo: String(index).padStart(5, '0'),
      caseId: this.makeCaseId(index),
      pan: pan.toUpperCase(),
      name,
      dobDoi: v(['DOB/DOI', 'DOB', 'Date of Birth']),
      mobile: v(['Mobile', 'Mobile Number', 'Phone']),
      email: v(['E-Mail', 'Email', 'E-Mail Address']),
      pinCode: v(['PIN Code', 'Pincode']),
      address: v(['Address']),
      state: v(['State', 'State/UT', 'State UT Code']),
      verificationStatus: (v(['Verification Status']) || 'Pending') as 'Pending' | 'Completed',
      informationDetails: {
        informationFy: v(['Information FY', 'FY']),
        informationSourceType: v(['Information Source Type']),
        informationSourceDescription: v(['Information Source Description']),
        informationType: v(['Information Type']),
        informationDescription: v(['Information Description']),
        informationValue: v(['Information Value']),
        source: v(['Source']),
        finding: v(['Finding'])
      },
      verificationDetails: {
        actionableAy: v(['Actionable AY']),
        statutoryReason: v(['Statutory Reason']),
        verificationResultType: v(['Verification Result Type']),
        incomeEscapingAssessmentValue: v(['Income Escaping Assessment Value']),
        informationValue: v(['Verification Information Value']),
        resultDescription: v(['Verification Result Description'])
      }
    };
  }

  private parseCsv(text: string): Array<Record<string, string>> {
    const lines: string[][] = [];
    let row: string[] = [];
    let field = '';
    let quoted = false;
    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      const next = text[i + 1];
      if (quoted && ch === '"' && next === '"') { field += '"'; i++; continue; }
      if (ch === '"') { quoted = !quoted; continue; }
      if (!quoted && ch === ',') { row.push(field); field = ''; continue; }
      if (!quoted && (ch === '\n' || ch === '\r')) {
        if (ch === '\r' && next === '\n') i++;
        row.push(field); field = '';
        if (row.some(v => v.trim() !== '')) lines.push(row);
        row = [];
        continue;
      }
      field += ch;
    }
    if (field.length || row.length) { row.push(field); if (row.some(v => v.trim() !== '')) lines.push(row); }
    if (lines.length < 2) return [];
    const headers = lines[0].map(h => h.trim());
    return lines.slice(1).map(values => Object.fromEntries(headers.map((h, i) => [h, values[i] ?? ''])));
  }

  private normalHeader(value: string): string {
    return value.trim().toLowerCase().replace(/[\s_\-./]+/g, '');
  }

  private makeCaseId(index: number): string {
    const ref = this.packet.referenceNumber.trim() || 'LOCAL';
    return `${ref}-${String(index).padStart(5, '0')}`;
  }

  private async restoreLocalDraft(): Promise<void> {
    try {
      const bundle = await this.drafts.load();
      if (bundle) {
        this.lastSavedAt.set(bundle.draft.savedAt);
      }
    } catch {
      // IndexedDB may be unavailable in restricted/private webviews; the rest of the UI remains usable.
    }
  }

  private applyDraft(draft: DraftState, files: Record<string, { name: string; type: string; lastModified: number; file: File }> = {}): void {
    this.packetForm.patchValue(draft.packet);
    this.rows.set(draft.rows);
    this.documents.set(draft.documents.map(doc => ({ ...doc, file: files[doc.id]?.file })));
    this.step.set(Math.min(3, Math.max(1, draft.step)));
    this.page.set(1);
    this.lastSavedAt.set(draft.savedAt);
  }

  private resetApplication(): void {
    this.packetForm.reset();
    this.rows.set([]);
    this.documents.set([]);
    this.selectedRows.set(new Set());
    this.currentRowIndex.set(null);
    this.page.set(1);
    this.filter.set('');
    this.lastSavedAt.set(null);
    this.progressMessage.set('');
    this.modal.set('none');
  }

  private onlineWatcher(): void {
    const update = () => this.isOnline.set(navigator.onLine);
    update();
    window.addEventListener('online', update);
    window.addEventListener('offline', update);
    window.setInterval(async () => {
      if (!navigator.onLine) { this.isOnline.set(false); return; }
      this.isOnline.set(await this.api.health());
    }, 10000);
  }

  private downloadBlob(blob: Blob, name: string): void {
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  }
}
