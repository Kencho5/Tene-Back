# Checkout & Payments API

Base URL: `http://localhost:3000` (dev)

---

## 1. Create Checkout

Creates an order and returns a Flitt hosted payment page URL.

**`POST /checkout`**

**Headers:**
```
Authorization: Bearer <token>
Content-Type: application/json
```

**Request Body:**
```json
{
  "customer_type": "individual",
  "individual": {
    "name": "giorgi",
    "surname": "kenchadze"
  },
  "company": null,
  "email": "giokenchadze@gmail.com",
  "phone_number": 557325235,
  "address": "tbilisi, rustaveli 12",
  "delivery_type": "delivery",
  "delivery_time": "same_day",
  "items": [
    { "product_id": 1, "quantity": 2 },
    { "product_id": 5, "quantity": 1 }
  ]
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `customer_type` | string | yes | `"individual"` or `"company"` |
| `individual` | object \| null | if individual | `{ name, surname }` |
| `company` | object \| null | if company | `{ organization_type, organization_name, organization_code }` |
| `email` | string | yes | Must contain `@` |
| `phone_number` | number | yes | |
| `address` | string | yes | Non-empty |
| `delivery_type` | string | yes | e.g. `"delivery"`, `"pickup"` |
| `delivery_time` | string | yes | e.g. `"same_day"`, `"next_day"` |
| `items` | array | yes | At least 1 item |
| `items[].product_id` | number | yes | Must exist and be enabled |
| `items[].quantity` | number | yes | Must be > 0, must not exceed stock |

**Success Response (200):**
```json
{
  "order_id": "tene_a1b2c3d4-...",
  "checkout_url": "https://pay.flitt.com/merchants/1549901/..."
}
```

**After receiving the response**, redirect the user to `checkout_url`:
```ts
window.location.href = response.checkout_url;
```

**Error Responses:**

| Status | Example message |
|--------|----------------|
| 400 | `"Cart is empty"`, `"Product 5 is not available"`, `"Insufficient stock for product 1"` |
| 401 | `"Authentication required"` |
| 404 | `"Product 99 not found"` |

---

## 2. Get User Orders

Returns all orders for the authenticated user, newest first.

**`GET /orders`**

**Headers:**
```
Authorization: Bearer <token>
```

**Success Response (200):**
```json
[
  {
    "id": 1,
    "user_id": 42,
    "order_id": "tene_a1b2c3d4-...",
    "status": "approved",
    "payment_id": 805243692,
    "amount": 15050,
    "currency": "GEL",
    "customer_type": "individual",
    "customer_name": "giorgi",
    "customer_surname": "kenchadze",
    "organization_type": null,
    "organization_name": null,
    "organization_code": null,
    "email": "giokenchadze@gmail.com",
    "phone_number": 557325235,
    "address": "tbilisi, rustaveli 12",
    "delivery_type": "delivery",
    "delivery_time": "same_day",
    "checkout_url": "https://pay.flitt.com/merchants/...",
    "created_at": "2026-02-22T12:00:00Z",
    "updated_at": "2026-02-22T12:01:30Z"
  }
]
```

**Order statuses:**

| Status | Meaning |
|--------|---------|
| `pending` | Order created, awaiting payment |
| `approved` | Payment successful |
| `declined` | Payment declined |
| `expired` | Payment page expired (not completed in time) |
| `processing` | Payment is being processed |

> **Note:** `amount` is in tetri (smallest unit). Divide by 100 to get GEL.
> Example: `15050` = **150.50 GEL**

---

## 3. Payment Callback (internal)

This endpoint is called server-to-server by Flitt. Frontend does not call this.

**`POST /payments/callback`** — no auth, signature-verified.

---

## Checkout Flow

```
Frontend                    Backend                     Flitt
   |                          |                           |
   |-- POST /checkout ------->|                           |
   |                          |-- POST /api/checkout/url ->|
   |                          |<-- { checkout_url } -------|
   |<-- { checkout_url } -----|                           |
   |                          |                           |
   |-- redirect to checkout_url ------------------------->|
   |                          |                           |
   |          (user pays on Flitt hosted page)            |
   |                          |                           |
   |                          |<-- POST /payments/callback |
   |                          |    (order_status update)   |
   |                          |-- 200 OK ---------------->|
   |                          |                           |
   |<-- redirect to /checkout/result ---------------------|
   |                          |                           |
   |-- GET /orders ---------->|                           |
   |<-- [{ status, ... }] ----|                           |
```

### Frontend implementation steps:

1. Collect cart items + customer info in checkout form
2. `POST /checkout` with the data
3. Redirect user to `checkout_url` from the response
4. After payment, Flitt redirects user back to `{FRONTEND_URL}/checkout/result`
5. On the result page, call `GET /orders` to show the order status
