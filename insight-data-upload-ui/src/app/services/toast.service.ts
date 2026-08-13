import { Injectable, signal } from '@angular/core';

export type ToastKind = 'success' | 'error' | 'info';
export interface Toast { id: number; kind: ToastKind; message: string; }

@Injectable({ providedIn: 'root' })
export class ToastService {
  private seq = 0;
  readonly toasts = signal<Toast[]>([]);

  show(message: string, kind: ToastKind = 'info', ms = 3500): void {
    const id = ++this.seq;
    this.toasts.update(list => [...list, { id, kind, message }]);
    window.setTimeout(() => this.remove(id), ms);
  }

  remove(id: number): void {
    this.toasts.update(list => list.filter(t => t.id !== id));
  }
}
