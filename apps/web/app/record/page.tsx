'use client';
import { Suspense } from 'react';
import { useSearchParams } from 'next/navigation';
import { RecordStudio } from '../../components/record-studio';

function Record() {
  const role = useSearchParams().get('role') || 'sept';
  return <RecordStudio roleId={role} />;
}

export default function RecordPage() {
  return <Suspense fallback={<main className="studio">Loading the record…</main>}><Record /></Suspense>;
}
