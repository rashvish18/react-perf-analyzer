// no_postmessage_wildcard test fixtures
// BAD cases — should be flagged

// window.postMessage with wildcard
export const Bad1 = (data: any) => {
  window.postMessage(data, "*");
};

// iframe postMessage with wildcard
export const Bad2 = (iframeRef: any, token: string) => {
  iframeRef.current.contentWindow.postMessage({ token }, "*");
};

// Inside useEffect
import { useEffect } from "react";
export const Bad3 = () => {
  useEffect(() => {
    window.postMessage({ type: "INIT" }, "*");
  }, []);
  return null;
};

// GOOD cases — should NOT be flagged

// Specific origin
export const Good1 = (data: any) => {
  window.postMessage(data, "https://app.example.com");
};

// Dynamic origin from config
export const Good2 = (data: any, origin: string) => {
  window.postMessage(data, origin);
};

// window.location.origin
export const Good3 = (data: any) => {
  window.postMessage(data, window.location.origin);
};
