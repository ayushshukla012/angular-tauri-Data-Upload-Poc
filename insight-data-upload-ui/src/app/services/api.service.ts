import { Injectable, inject } from '@angular/core';
import { HttpClient, HttpErrorResponse, HttpHeaders } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { environment } from '../../environments/environment';
import { AuthService } from './auth.service';
import {
  CaseResponse,
  InitiateUploadResponse,
  PacketDetails,
  PacketResponse,
  PersonRow,
  PresignPartResponse,
  UploadedPartSummary
} from '../models';

@Injectable({ providedIn: 'root' })
export class ApiService {
  private readonly http = inject(HttpClient);
  private readonly auth = inject(AuthService);
  private readonly bffBase = (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window)
    ? environment.bffBaseUrl.replace(/\/$/, '')
    : '/bff';

  private proxy(path: string): string {
    return `${this.bffBase}/api/bff/proxy${path.startsWith('/') ? path : `/${path}`}`;
  }

  private authHeaders(): HttpHeaders {
    const token = this.auth.getAccessToken();
    return token ? new HttpHeaders({ Authorization: `Bearer ${token}` }) : new HttpHeaders();
  }

  async health(): Promise<boolean> {
    try {
      await firstValueFrom(this.http.get<{ status: string }>(`${this.bffBase}/actuator/health`));
      return true;
    } catch {
      return false;
    }
  }

  async submitPacket(packet: PacketDetails): Promise<PacketResponse> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required to create a packet.');
    return firstValueFrom(this.http.post<PacketResponse>(this.proxy('/api/v1/packets'), {
      batchNumber: packet.referenceNumber,
      description: packet.description,
      submittingPersonName: packet.submittingPersonName,
      submittingPersonAddress: packet.submittingPersonAddress,
      submittingPersonMobile: packet.submittingPersonMobile,
      submittingPersonEmail: packet.submittingPersonEmail
    }, { headers: this.authHeaders(), withCredentials: true }));
  }

  async submitCase(row: PersonRow, packet: PacketDetails): Promise<CaseResponse> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required to submit case records.');
    const body = {
      caseId: row.caseId,
      referenceNumber: packet.referenceNumber,
      sourcePan: row.pan,
      name: row.name,
      dateOfBirth: row.dobDoi,
      address: row.address,
      stateUtCode: row.state,
      pincode: row.pinCode,
      mobileNumber: row.mobile,
      email: row.email,
      designation: environment.defaultCaseDesignation,
      informationFy: row.informationDetails.informationFy || null,
      informationSourceType: row.informationDetails.informationSourceType || null,
      informationSourceDescription: row.informationDetails.informationSourceDescription || null,
      informationType: row.informationDetails.informationType || null,
      informationDescription: row.informationDetails.informationDescription || null,
      informationValue: row.informationDetails.informationValue || null,
      natureOfVerification: row.verificationDetails.statutoryReason || null,
      actionableAy: row.verificationDetails.actionableAy || null,
      verificationResultType1: row.verificationDetails.verificationResultType || null,
      verificationResultDescription1: row.verificationDetails.resultDescription || null,
      verificationResultValue1: row.verificationDetails.incomeEscapingAssessmentValue || null,
      batchNumber: packet.referenceNumber,
      extraFields: {
        verificationStatus: row.verificationStatus,
        informationSource: row.informationDetails.source,
        informationFinding: row.informationDetails.finding,
        verificationInformationValue: row.verificationDetails.informationValue
      }
    };

    return firstValueFrom(this.http.post<CaseResponse>(this.proxy('/api/v1/cases'), body, { headers: this.authHeaders(), withCredentials: true }));
  }

  async initiateUpload(file: File, caseId?: string, doc?: { label: string; type: string; description: string; remarks: string }): Promise<InitiateUploadResponse> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required to upload supporting documents.');
    return firstValueFrom(this.http.post<InitiateUploadResponse>(this.proxy('/api/v1/uploads/initiate'), {
      fileName: file.name,
      fileSizeBytes: file.size,
      caseId: caseId || null,
      docLabel: doc?.label || null,
      docType: doc?.type || null,
      description: doc?.description || null,
      remarks: doc?.remarks || null
    }, { headers: this.authHeaders(), withCredentials: true }));
  }

  async uploadSingle(file: File, init: InitiateUploadResponse): Promise<void> {
    if (!init.uploadUrl || !init.contentType) throw new Error('Upload service did not return a signed upload URL.');
    const headers = new HttpHeaders({ 'Content-Type': init.contentType });
    await firstValueFrom(this.http.put(init.uploadUrl, file, { headers, responseType: 'text' }));
  }

  async presignPart(uploadId: string, partNumber: number): Promise<PresignPartResponse> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required for multipart upload.');
    return firstValueFrom(this.http.post<PresignPartResponse>(this.proxy(`/api/v1/uploads/${encodeURIComponent(uploadId)}/parts/${partNumber}/presign`), {}, { headers: this.authHeaders(), withCredentials: true }));
  }

  async listParts(uploadId: string): Promise<UploadedPartSummary[]> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required for multipart upload.');
    const response = await firstValueFrom(this.http.get<{ parts: UploadedPartSummary[] }>(this.proxy(`/api/v1/uploads/${encodeURIComponent(uploadId)}/parts`), { headers: this.authHeaders(), withCredentials: true }));
    return response.parts;
  }

  async completeUpload(uploadId: string): Promise<void> {
    if (!(await this.auth.ensureValidAccessToken())) throw new Error('Authentication is required to complete the upload.');
    await firstValueFrom(this.http.post(this.proxy(`/api/v1/uploads/${encodeURIComponent(uploadId)}/complete`), {}, { headers: this.authHeaders(), withCredentials: true }));
  }

  async uploadFile(file: File, caseId: string | undefined, doc?: { label: string; type: string; description: string; remarks: string }): Promise<void> {
    const init = await this.initiateUpload(file, caseId, doc);
    if (!init.multipart) {
      await this.uploadSingle(file, init);
      await this.completeUpload(init.uploadId);
      return;
    }
    await this.uploadMultipart(file, init);
  }

  private async uploadMultipart(file: File, init: InitiateUploadResponse): Promise<void> {
    const partSize = Math.max(5 * 1024 * 1024, init.recommendedPartSizeBytes || 5 * 1024 * 1024);
    const existing = new Map<number, UploadedPartSummary>((await this.listParts(init.uploadId)).map(p => [p.partNumber, p]));
    const totalParts = Math.ceil(file.size / partSize);
    const completed = new Map<number, { eTag: string; sizeBytes: number }>();

    for (let partNumber = 1; partNumber <= totalParts; partNumber++) {
      if (existing.has(partNumber)) {
        const part = existing.get(partNumber)!;
        completed.set(partNumber, { eTag: part.eTag, sizeBytes: part.sizeBytes });
        continue;
      }
      const start = (partNumber - 1) * partSize;
      const end = Math.min(file.size, start + partSize);
      const blob = file.slice(start, end);
      const presign = await this.presignPart(init.uploadId, partNumber);
      const response = await firstValueFrom(this.http.put(presign.uploadUrl, blob, { observe: 'response', responseType: 'text' }));
      const eTag = response.headers.get('ETag')?.replaceAll('"', '') || '';
      completed.set(partNumber, { eTag, sizeBytes: blob.size });
    }

    // The provided backend contract exposes part discovery and presigning but the complete call
    // assembles server-side from the stored parts, so no extra part payload is sent here.
    if (completed.size !== totalParts) throw new Error('Multipart upload did not complete all parts.');
    await this.completeUpload(init.uploadId);
  }

  describeError(error: unknown): string {
    if (error instanceof HttpErrorResponse) {
      if (error.status === 0) {
        return 'Unable to connect to the server. Please ensure the backend service is running and try again.';
      }
      if (error.status === 401) return 'Your sign-in session has expired. Please sign in again.';
      if (error.status === 403) return 'You are authenticated but are not authorized for this operation.';
      const api = error.error as { message?: string; code?: string } | undefined;
      return api?.message || api?.code || `Request failed with HTTP ${error.status}.`;
    }
    if (error instanceof Error && (error.message === 'Failed to fetch' || error.message.includes('NetworkError') || error.message.includes('ERR_CONNECTION_REFUSED'))) {
      return 'Unable to connect to the server. Please ensure the backend service is running and try again.';
    }
    return error instanceof Error ? error.message : 'An unexpected error occurred.';
  }
}
